//! 编译服务（移植 deprecated core/llm.py 的 compile_documents）。
//!
//! 对 status='raw' 的文档逐个调用 LLM（docs_compile 提示词），把编译稿与
//! 元数据写回 documents 表并流转状态；失败写 last_error、状态不前进、
//! 不中断批次。LLM 通过 `LlmClient` trait 注入，测试用确定性 fake。

use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::documents;
use crate::llm::{parse_response, CompileOutcome, LlmClient, LlmConfig, OpenAiClient};
use crate::settings;

pub const PROMPT_KEY: &str = "prompt:docs_compile";

const SETTING_LLM_BASE_URL: &str = "llm_base_url";
const SETTING_LLM_API_KEY: &str = "llm_api_key";
const SETTING_LLM_MODEL: &str = "llm_model";

/// 从 settings 组装 OpenAI 兼容客户端；三键任一缺失/为空即报错。
pub fn client_from_settings(vault: &Path) -> Result<OpenAiClient, String> {
    let non_empty = |value: Option<String>| value.filter(|v| !v.trim().is_empty());
    let base_url = non_empty(settings::get(vault, SETTING_LLM_BASE_URL)?);
    let api_key = non_empty(settings::get(vault, SETTING_LLM_API_KEY)?);
    let model = non_empty(settings::get(vault, SETTING_LLM_MODEL)?);
    match (base_url, api_key, model) {
        (Some(base_url), Some(api_key), Some(model)) => Ok(OpenAiClient::new(LlmConfig {
            base_url,
            api_key,
            model,
        })),
        _ => Err("LLM 未配置：请先在设置中填写 Base URL、API Key 与模型".to_string()),
    }
}

/// 编译提示词：设置键 `prompt:docs_compile` 的定制值优先（非空），否则用打包内置默认。
pub fn resolve_prompt(vault: &Path) -> Result<String, String> {
    if let Some(custom) = settings::get(vault, PROMPT_KEY)? {
        if !custom.trim().is_empty() {
            return Ok(custom);
        }
    }
    settings::default_prompt(settings::PROMPT_DOCS_COMPILE)
        .map(str::to_string)
        .ok_or_else(|| "内置提示词缺失".to_string())
}

/// 编译文档：空 id 列表选择全部 raw；显式 id 不存在则产出错误结果（不中断批次）。
/// 返回前一次性提交所有写库（对齐旧版单 session 末尾 commit）。
pub fn compile_documents(
    vault: &Path,
    document_ids: &[String],
    client: &dyn LlmClient,
) -> Result<Vec<CompileOutcome>, String> {
    let mut conn = Connection::open(crate::db::db_path(vault)).map_err(|e| format!("打开数据库失败: {e}"))?;
    documents::ensure_table(&conn)?;
    let prompt_template = resolve_prompt(vault)?;

    let mut outcomes: Vec<CompileOutcome> = Vec::new();
    let tx = conn.transaction().map_err(|e| format!("开启事务失败: {e}"))?;

    if document_ids.is_empty() {
        let targets = select_by_status(&tx, "raw")?;
        for (id, content) in targets {
            outcomes.push(compile_one(&tx, client, &prompt_template, &id, &content));
        }
    } else {
        for requested in document_ids {
            let target: Option<(String, String, String)> = tx
                .query_row(
                    "SELECT id, content, file_name FROM documents WHERE id = ?1",
                    [requested],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()
                .map_err(|e| format!("查询失败: {e}"))?;
            match target {
                Some((id, content, _file_name)) => {
                    outcomes.push(compile_one(&tx, client, &prompt_template, &id, &content));
                }
                None => outcomes.push(CompileOutcome {
                    document_id: requested.clone(),
                    title: None,
                    tags: None,
                    ok: false,
                    error: Some("document not found".to_string()),
                }),
            }
        }
    }

    tx.commit().map_err(|e| format!("提交失败: {e}"))?;
    Ok(outcomes)
}

fn select_by_status(tx: &rusqlite::Transaction, status: &str) -> Result<Vec<(String, String)>, String> {
    let mut stmt = tx
        .prepare("SELECT id, content FROM documents WHERE status = ?1 ORDER BY created_at")
        .map_err(|e| format!("查询失败: {e}"))?;
    let rows = stmt
        .query_map([status], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|e| format!("查询失败: {e}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("读取结果失败: {e}"))
}

/// 编译单个文档：成功更新编译稿与状态，失败写 last_error（状态不前进）。
fn compile_one(
    tx: &rusqlite::Transaction,
    client: &dyn LlmClient,
    template: &str,
    id: &str,
    content: &str,
) -> CompileOutcome {
    let current: Option<String> = tx
        .query_row("SELECT status FROM documents WHERE id = ?1", [id], |row| row.get(0))
        .optional()
        .unwrap_or(None);
    if current.as_deref().is_some_and(|status| status != "raw") {
        // 已编译：跳过并返回既有元数据标题（对齐旧版 skip 行为）
        let (_, title, _tags) = compiled_metadata(tx, id);
        return CompileOutcome {
            document_id: id.to_string(),
            title,
            tags: None,
            ok: true,
            error: None,
        };
    }

    let prompt = template.replace("{content}", content);
    let parsed = match client.chat(&prompt).and_then(|response| parse_response(&response)) {
        Ok((Value::Object(metadata), body)) => Ok((metadata, body)),
        Ok(_) => Err("LLM 响应元数据格式异常".to_string()),
        Err(error) => Err(error),
    };
    match parsed {
        Err(error) => {
            let message = format!("compile: {error}");
            crate::logging::log("ERROR", &format!("文档编译失败 {id}: {message}"));
            let _ = tx.execute(
                "UPDATE documents SET last_error = ?2, updated_at = ?3 WHERE id = ?1",
                params![id, message, now_rfc3339()],
            );
            CompileOutcome {
                document_id: id.to_string(),
                title: None,
                tags: None,
                ok: false,
                error: Some(error),
            }
        }
        Ok((mut metadata, body)) => {
            let file_name: Option<String> = tx
                .query_row("SELECT file_name FROM documents WHERE id = ?1", [id], |row| row.get(0))
                .optional()
                .unwrap_or(None);
            // title 回退链：front matter 标题 → file_name（对齐旧版 setdefault）
            if metadata.get("title").and_then(Value::as_str).map(str::is_empty) != Some(false) {
                if let Some(fallback) = file_name {
                    metadata.insert("title".to_string(), Value::String(fallback));
                }
            }
            let title = metadata.get("title").and_then(Value::as_str).map(str::to_string);
            let tags = metadata.get("tags").and_then(|value| match value {
                Value::Array(items) => Some(
                    items
                        .iter()
                        .filter_map(|item| item.as_str().map(str::to_string))
                        .collect::<Vec<String>>(),
                ),
                Value::String(text) => Some(vec![text.clone()]),
                _ => None,
            });
            let serialized = serde_json::to_string(&Value::Object(metadata)).unwrap_or_else(|_| "{}".to_string());
            let now = now_rfc3339();
            if let Err(error) = tx.execute(
                "UPDATE documents
                 SET metadata_compiled = ?2, content_compiled = ?3, status = 'compiled',
                     compiled_at = ?4, llm_model = ?5, last_error = NULL, updated_at = ?4
                 WHERE id = ?1",
                params![id, serialized, body, now, client.model()],
            ) {
                return CompileOutcome {
                    document_id: id.to_string(),
                    title,
                    tags,
                    ok: false,
                    error: Some(format!("写入数据库失败: {error}")),
                };
            }
            CompileOutcome {
                document_id: id.to_string(),
                title,
                tags,
                ok: true,
                error: None,
            }
        }
    }
}

/// 读取已编译文档的元数据标题（无元数据或解析失败为 None）。
fn compiled_metadata(tx: &rusqlite::Transaction, id: &str) -> (Option<Value>, Option<String>, Option<Vec<String>>) {
    let raw: Option<String> = tx
        .query_row("SELECT metadata_compiled FROM documents WHERE id = ?1", [id], |row| {
            row.get(0)
        })
        .optional()
        .unwrap_or(None);
    let Some(raw) = raw else {
        return (None, None, None);
    };
    match serde_json::from_str::<Value>(&raw) {
        Ok(value) => {
            let title = value.get("title").and_then(Value::as_str).map(str::to_string);
            let tags = value
                .get("tags")
                .and_then(Value::as_array)
                .map(|items| items.iter().filter_map(|i| i.as_str().map(str::to_string)).collect());
            (Some(value), title, tags)
        }
        Err(_) => (None, None, None),
    }
}

fn now_rfc3339() -> String {
    OffsetDateTime::now_utc().format(&Rfc3339).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::ensure_db;
    use std::sync::Mutex;
    use tempfile::TempDir;

    /// 确定性 fake：按序返回预置响应并记录调用（Mutex 以满足 LlmClient: Sync）。
    struct FakeClient {
        replies: Mutex<Vec<Result<String, String>>>,
        prompts: Mutex<Vec<String>>,
    }

    impl FakeClient {
        fn new(replies: Vec<Result<String, String>>) -> Self {
            Self {
                replies: Mutex::new(replies),
                prompts: Mutex::new(Vec::new()),
            }
        }
    }

    impl LlmClient for FakeClient {
        fn model(&self) -> &str {
            "fake-model"
        }

        fn chat(&self, prompt: &str) -> Result<String, String> {
            self.prompts.lock().unwrap().push(prompt.to_string());
            self.replies
                .lock()
                .unwrap()
                .pop()
                .unwrap_or_else(|| Err("无预置响应".to_string()))
        }
    }

    fn setup() -> TempDir {
        let tmp = TempDir::new().unwrap();
        ensure_db(tmp.path()).unwrap();
        tmp
    }

    fn add_doc(vault: &Path, name: &str, content: &str) -> String {
        let file = vault.join(name);
        std::fs::write(&file, content).unwrap();
        let conn = Connection::open(crate::db::db_path(vault)).unwrap();
        let id = format!("id-{}", name.replace('.', "-"));
        conn.execute(
            "INSERT OR REPLACE INTO documents (id, file_name, mime_type, type, sha1, size, content, status, created_at, updated_at)
             VALUES (?1, ?2, 'text/markdown', 'plaintext', ?3, 1, ?4, 'raw', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            params![id, name, name, content],
        )
        .unwrap();
        id
    }

    fn row(vault: &Path, id: &str) -> (String, String, String, Option<String>, String) {
        let conn = Connection::open(crate::db::db_path(vault)).unwrap();
        conn.query_row(
            "SELECT status, COALESCE(content_compiled, ''), COALESCE(metadata_compiled, ''), last_error, COALESCE(llm_model, '')
             FROM documents WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .unwrap()
    }

    const GOOD_RESPONSE: &str = "---\ntitle: 笔记标题\nsummary: 一句话\nlanguage: zh\ntags: [a, b]\n---\n\n# 笔记标题\n\n<!-- chunk: 正文 -->\n内容";

    #[test]
    fn test_compile_updates_document_and_status() {
        let tmp = setup();
        let id = add_doc(tmp.path(), "a.md", "原始内容");
        let client = FakeClient::new(vec![Ok(GOOD_RESPONSE.to_string())]);

        let outcomes = compile_documents(tmp.path(), std::slice::from_ref(&id), &client).unwrap();

        assert!(outcomes[0].ok);
        assert_eq!(outcomes[0].title.as_deref(), Some("笔记标题"));
        assert_eq!(outcomes[0].tags.clone(), Some(vec!["a".to_string(), "b".to_string()]));
        let (status, compiled, metadata, last_error, llm_model) = row(tmp.path(), &id);
        assert_eq!(status, "compiled");
        assert!(compiled.contains("<!-- chunk: 正文 -->"));
        assert!(metadata.contains("\"title\""));
        assert_eq!(last_error, None);
        assert_eq!(llm_model, "fake-model");
        // 提示词包含替换后的内容
        let prompts = client.prompts.lock().unwrap();
        assert!(prompts[0].contains("原始内容"));
        assert!(!prompts[0].contains("{content}"));
    }

    #[test]
    fn test_compile_failure_writes_last_error_and_keeps_raw() {
        let tmp = setup();
        let id = add_doc(tmp.path(), "a.md", "原始内容");
        let client = FakeClient::new(vec![Err("网络错误".to_string())]);

        let outcomes = compile_documents(tmp.path(), std::slice::from_ref(&id), &client).unwrap();

        assert!(!outcomes[0].ok);
        assert_eq!(outcomes[0].error.as_deref(), Some("网络错误"));
        let (status, _, _, last_error, _) = row(tmp.path(), &id);
        assert_eq!(status, "raw");
        assert_eq!(last_error.as_deref(), Some("compile: 网络错误"));
    }

    #[test]
    fn test_compile_empty_ids_selects_raw_only_and_skips_compiled() {
        let tmp = setup();
        let first = add_doc(tmp.path(), "a.md", "甲");
        let second = add_doc(tmp.path(), "b.md", "乙");
        // 先把第一个编译掉
        let client = FakeClient::new(vec![Ok(GOOD_RESPONSE.to_string())]);
        compile_documents(tmp.path(), std::slice::from_ref(&first), &client).unwrap();

        // 空列表只应处理剩余 raw（乙），fake 只有一个响应
        let client = FakeClient::new(vec![Ok(GOOD_RESPONSE.to_string())]);
        let outcomes = compile_documents(tmp.path(), &[], &client).unwrap();

        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].document_id, second);
        assert!(outcomes[0].ok);
    }

    #[test]
    fn test_compile_missing_id_yields_error_outcome() {
        let tmp = setup();
        let client = FakeClient::new(vec![]);

        let outcomes = compile_documents(tmp.path(), &["nope".to_string()], &client).unwrap();

        assert!(!outcomes[0].ok);
        assert_eq!(outcomes[0].error.as_deref(), Some("document not found"));
    }

    #[test]
    fn test_compile_title_falls_back_to_file_name() {
        let tmp = setup();
        let id = add_doc(tmp.path(), "草稿.md", "内容");
        let client = FakeClient::new(vec![Ok("没有 front matter 也没有标题".to_string())]);

        let outcomes = compile_documents(tmp.path(), std::slice::from_ref(&id), &client).unwrap();

        assert!(outcomes[0].ok);
        assert_eq!(outcomes[0].title.as_deref(), Some("草稿.md"));
        let (_, _, metadata, _, _) = row(tmp.path(), &id);
        assert!(metadata.contains("草稿.md"));
    }

    #[test]
    fn test_resolve_prompt_prefers_custom_settings() {
        let tmp = setup();
        // 未定制 → 内置默认（含 {content} 占位符）
        let default = resolve_prompt(tmp.path()).unwrap();
        assert!(default.contains("{content}"));

        settings::set(tmp.path(), PROMPT_KEY, Some("定制模板 {content}".into())).unwrap();
        let custom = resolve_prompt(tmp.path()).unwrap();
        assert_eq!(custom, "定制模板 {content}");
    }

    #[test]
    fn test_resolve_prompt_ignores_blank_custom() {
        let tmp = setup();
        settings::set(tmp.path(), PROMPT_KEY, Some("   ".into())).unwrap();
        let prompt = resolve_prompt(tmp.path()).unwrap();
        assert!(prompt.contains("{content}"));
    }

    #[test]
    fn test_client_from_settings_requires_all_keys() {
        let tmp = setup();
        assert!(client_from_settings(tmp.path()).is_err());

        settings::set(tmp.path(), "llm_base_url", Some("https://api.example.com/v1".into())).unwrap();
        settings::set(tmp.path(), "llm_api_key", Some("sk-x".into())).unwrap();
        settings::set(tmp.path(), "llm_model", Some("gpt-5".into())).unwrap();
        assert!(client_from_settings(tmp.path()).is_ok());

        // 任一键置空也算未配置
        settings::set(tmp.path(), "llm_api_key", Some("   ".into())).unwrap();
        assert!(client_from_settings(tmp.path()).is_err());
    }

    #[test]
    fn test_compile_failure_then_success_recovers() {
        let tmp = setup();
        let id = add_doc(tmp.path(), "a.md", "内容");

        let failing = FakeClient::new(vec![Err("第一次失败".to_string())]);
        compile_documents(tmp.path(), std::slice::from_ref(&id), &failing).unwrap();

        let ok = FakeClient::new(vec![Ok(GOOD_RESPONSE.to_string())]);
        let outcomes = compile_documents(tmp.path(), std::slice::from_ref(&id), &ok).unwrap();

        assert!(outcomes[0].ok);
        let (status, _, _, last_error, _) = row(tmp.path(), &id);
        assert_eq!(status, "compiled");
        assert_eq!(last_error, None);
    }
}
