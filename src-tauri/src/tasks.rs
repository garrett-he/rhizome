//! 任务中心：批量编译/索引编排，逐文档推送进度事件并记录 jobs 历史。

use std::path::Path;

use serde::Serialize;

use crate::compile;
use crate::embedding::{self, EmbeddingModel};
use crate::jobs;
use crate::llm::LlmClient;
use crate::settings;

pub const STEP_COMPILE: &str = "compile";
pub const STEP_INDEX: &str = "index";
pub const STEP_ALL: &str = "all";

const SETTING_EMBEDDING_MODEL: &str = "embedding_model";

/// 任务进度事件（Tauri 侧转发为 task-start / task-item / task-done）。
pub trait TaskEvents: Send + Sync {
    fn start(&self, job_id: &str, kind: &str, total: u64);
    fn item(&self, job_id: &str, document_id: &str, step: &str, ok: bool, error: Option<&str>);
    fn done(&self, job_id: &str, succeeded: u64, failed: u64);
}

/// 一次批量处理的汇总（命令返回值）。
#[derive(Serialize, Debug, PartialEq)]
pub struct TaskSummary {
    pub job_id: String,
    pub kind: String,
    pub total: u64,
    pub succeeded: u64,
    pub failed: u64,
}

struct Target {
    id: String,
    #[allow(dead_code)]
    file_name: String,
}

fn validate_step(step: &str) -> Result<(), String> {
    match step {
        STEP_COMPILE | STEP_INDEX | STEP_ALL => Ok(()),
        other => Err(format!("未知步骤: {other}")),
    }
}

/// 选出目标文档：ids 非空按给定顺序；为空按步骤选择
/// （compile=全部 raw、index=全部 compiled、all=raw+compiled，均按导入时间升序）。
fn collect_targets(vault: &Path, ids: &[String], step: &str) -> Result<Vec<Target>, String> {
    let conn = crate::db::open_conn(vault)?;
    crate::documents::ensure_table(&conn)?;
    if !ids.is_empty() {
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!("SELECT id, file_name FROM documents WHERE id IN ({placeholders})");
        let mut stmt = conn.prepare(&sql).map_err(|e| format!("查询失败: {e}"))?;
        let mut found: Vec<(String, String)> = stmt
            .query_map(rusqlite::params_from_iter(ids.iter()), |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .map_err(|e| format!("查询失败: {e}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("读取结果失败: {e}"))?;
        found.sort_by(|a, b| {
            ids.iter()
                .position(|id| id == &a.0)
                .unwrap_or(usize::MAX)
                .cmp(&ids.iter().position(|id| id == &b.0).unwrap_or(usize::MAX))
        });
        return Ok(found
            .into_iter()
            .map(|(id, file_name)| Target { id, file_name })
            .collect());
    }
    let condition = match step {
        STEP_COMPILE => "status = 'raw'",
        STEP_INDEX => "status = 'compiled'",
        _ => "status IN ('raw', 'compiled')",
    };
    let sql = format!("SELECT id, file_name FROM documents WHERE {condition} ORDER BY created_at ASC");
    let mut stmt = conn.prepare(&sql).map_err(|e| format!("查询失败: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(Target {
                id: row.get(0)?,
                file_name: row.get(1)?,
            })
        })
        .map_err(|e| format!("查询失败: {e}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("读取结果失败: {e}"))
}

/// 批量执行：job_start → 逐文档执行 → 每步推送 item 事件（失败不中断）→ job_finish → done 事件。
/// client/model 由调用方注入；步骤不需要的传 None。
pub fn run_batch(
    vault: &Path,
    events: &dyn TaskEvents,
    ids: &[String],
    step: &str,
    client: Option<&dyn LlmClient>,
    model: Option<(&dyn EmbeddingModel, &str)>,
) -> Result<TaskSummary, String> {
    validate_step(step)?;
    let targets = collect_targets(vault, ids, step)?;
    let total = targets.len() as u64;
    let job = jobs::job_start(vault, step, total)?;
    events.start(&job.id, step, total);

    let mut succeeded = 0u64;
    let mut failed = 0u64;
    for target in &targets {
        let ok = match step {
            STEP_COMPILE => run_compile_step(vault, client, &target.id, &job.id, events),
            STEP_INDEX => run_index_step(vault, model, &target.id, &job.id, events),
            _ => {
                let compiled = run_compile_step(vault, client, &target.id, &job.id, events);
                compiled && run_index_step(vault, model, &target.id, &job.id, events)
            }
        };
        if ok {
            succeeded += 1;
        } else {
            failed += 1;
        }
    }
    jobs::job_finish(vault, &job.id, succeeded, failed)?;
    events.done(&job.id, succeeded, failed);
    Ok(TaskSummary {
        job_id: job.id,
        kind: job.kind,
        total,
        succeeded,
        failed,
    })
}

/// 从 settings 按需加载 LLM/embedding 依赖后执行批量（命令层入口）。
/// index 步骤无需 LLM；compile 步骤无需 embedding；LLM 未配置时返回 Err。
pub fn run_batch_from_settings(
    vault: &Path,
    events: &dyn TaskEvents,
    ids: &[String],
    step: &str,
) -> Result<TaskSummary, String> {
    validate_step(step)?;
    match step {
        STEP_INDEX => {
            let (model, name) = load_model(vault)?;
            run_batch(vault, events, ids, step, None, Some((model.as_ref(), &name)))
        }
        STEP_COMPILE => {
            let client = compile::client_from_settings(vault)?;
            run_batch(vault, events, ids, step, Some(&client), None)
        }
        _ => {
            let client = compile::client_from_settings(vault)?;
            let (model, name) = load_model(vault)?;
            run_batch(vault, events, ids, step, Some(&client), Some((model.as_ref(), &name)))
        }
    }
}

fn load_model(vault: &Path) -> Result<(std::sync::Arc<dyn EmbeddingModel>, String), String> {
    let name = settings::get(vault, SETTING_EMBEDDING_MODEL)?
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| embedding::SUPPORTED_MODEL.to_string());
    let model = embedding::default_embedding_model(&name)?;
    Ok((model, name))
}

fn run_compile_step(
    vault: &Path,
    client: Option<&dyn LlmClient>,
    id: &str,
    job_id: &str,
    events: &dyn TaskEvents,
) -> bool {
    let Some(client) = client else {
        events.item(job_id, id, STEP_COMPILE, false, Some("LLM 未配置"));
        return false;
    };
    let outcome = match compile::compile_documents(vault, std::slice::from_ref(&id.to_string()), client, false) {
        Ok(outcomes) => outcomes.first().cloned(),
        Err(e) => {
            events.item(job_id, id, STEP_COMPILE, false, Some(&e));
            return false;
        }
    };
    match outcome {
        Some(o) if o.ok => {
            events.item(job_id, id, STEP_COMPILE, true, None);
            true
        }
        Some(o) => {
            events.item(job_id, id, STEP_COMPILE, false, o.error.as_deref());
            false
        }
        None => {
            events.item(job_id, id, STEP_COMPILE, false, Some("无编译结果"));
            false
        }
    }
}

fn run_index_step(
    vault: &Path,
    model: Option<(&dyn EmbeddingModel, &str)>,
    id: &str,
    job_id: &str,
    events: &dyn TaskEvents,
) -> bool {
    let Some((model, name)) = model else {
        events.item(job_id, id, STEP_INDEX, false, Some("embedding 模型不可用"));
        return false;
    };
    let outcome = match crate::indexer::index_documents(vault, std::slice::from_ref(&id.to_string()), model, name) {
        Ok(outcomes) => outcomes.first().cloned(),
        Err(e) => {
            events.item(job_id, id, STEP_INDEX, false, Some(&e));
            return false;
        }
    };
    match outcome {
        Some(o) if o.ok => {
            events.item(job_id, id, STEP_INDEX, true, None);
            true
        }
        Some(o) => {
            events.item(job_id, id, STEP_INDEX, false, o.error.as_deref());
            false
        }
        None => {
            events.item(job_id, id, STEP_INDEX, false, Some("无索引结果"));
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::db::ensure_db;
    use rusqlite::Connection;
    use tempfile::TempDir;

    fn setup() -> TempDir {
        let tmp = TempDir::new().unwrap();
        ensure_db(tmp.path()).unwrap();
        tmp
    }

    /// 插入指定状态的文档行（content/编译稿按状态给最小数据）。
    fn insert_doc(vault: &Path, id: &str, status: &str, content_compiled: Option<&str>, last_error: Option<&str>) {
        let conn = Connection::open(crate::db::db_path(vault)).unwrap();
        conn.execute(
            "INSERT INTO documents (id, file_name, mime_type, type, sha1, size, content, status, created_at, updated_at, content_compiled, last_error)
             VALUES (?1, ?1, 'text/markdown', 'plaintext', ?2, 1, '原始内容', ?3, ?4, ?4, ?5, ?6)",
            rusqlite::params![id, format!("sha-{id}"), status, format!("2026-10-22T00:00:0{}Z", id.len() % 10), content_compiled, last_error],
        )
        .unwrap();
    }

    /// 捕获事件序列的 fake。
    #[derive(Default)]
    struct CapturedEvents {
        events: Mutex<Vec<String>>,
    }

    impl CapturedEvents {
        fn push(&self, line: String) {
            self.events.lock().unwrap().push(line);
        }

        fn lines(&self) -> Vec<String> {
            self.events.lock().unwrap().clone()
        }
    }

    impl TaskEvents for CapturedEvents {
        fn start(&self, job_id: &str, kind: &str, total: u64) {
            self.push(format!("start:{job_id}:{kind}:{total}"));
        }

        fn item(&self, job_id: &str, document_id: &str, step: &str, ok: bool, error: Option<&str>) {
            self.push(format!(
                "item:{job_id}:{document_id}:{step}:{ok}:{}",
                error.unwrap_or("-")
            ));
        }

        fn done(&self, job_id: &str, succeeded: u64, failed: u64) {
            self.push(format!("done:{job_id}:{succeeded}:{failed}"));
        }
    }

    /// 编译成功的 fake LLM（响应格式对齐 compile 模块解析规则）。
    struct OkClient;

    impl LlmClient for OkClient {
        fn model(&self) -> &str {
            "fake-model"
        }

        fn chat(&self, _prompt: &str) -> Result<String, String> {
            Ok("---\ntitle: 笔记标题\n---\n\n# 笔记标题\n\n<!-- chunk: 正文 -->\n内容".to_string())
        }
    }

    /// 编译失败的 fake LLM。
    struct FailingClient;

    impl LlmClient for FailingClient {
        fn model(&self) -> &str {
            "fake-model"
        }

        fn chat(&self, _prompt: &str) -> Result<String, String> {
            Err("LLM 炸了".to_string())
        }
    }

    #[test]
    fn test_validate_step_rejects_unknown() {
        assert_eq!(validate_step("nope").unwrap_err(), "未知步骤: nope");
    }

    #[test]
    fn test_collect_targets_selects_by_step() {
        let tmp = setup();
        insert_doc(tmp.path(), "raw1", "raw", None, None);
        insert_doc(tmp.path(), "compiled1", "compiled", Some("# 标题"), None);
        insert_doc(tmp.path(), "indexed1", "indexed", Some("# 标题"), None);

        let compile_ids: Vec<String> = collect_targets(tmp.path(), &[], STEP_COMPILE)
            .unwrap()
            .into_iter()
            .map(|t| t.id)
            .collect();
        assert_eq!(compile_ids, vec!["raw1"]);

        let index_ids: Vec<String> = collect_targets(tmp.path(), &[], STEP_INDEX)
            .unwrap()
            .into_iter()
            .map(|t| t.id)
            .collect();
        assert_eq!(index_ids, vec!["compiled1"]);

        let all_ids: Vec<String> = collect_targets(tmp.path(), &[], STEP_ALL)
            .unwrap()
            .into_iter()
            .map(|t| t.id)
            .collect();
        assert_eq!(all_ids, vec!["raw1", "compiled1"]);
    }

    #[test]
    fn test_collect_targets_preserves_explicit_order() {
        let tmp = setup();
        insert_doc(tmp.path(), "a", "raw", None, None);
        insert_doc(tmp.path(), "b", "raw", None, None);

        let targets = collect_targets(tmp.path(), &["b".to_string(), "a".to_string()], STEP_COMPILE).unwrap();
        let ids: Vec<String> = targets.into_iter().map(|t| t.id).collect();
        assert_eq!(ids, vec!["b", "a"]);
    }

    #[test]
    fn test_run_batch_index_with_fake_events_and_jobs() {
        let tmp = setup();
        insert_doc(
            tmp.path(),
            "c1",
            "compiled",
            Some("# 标题\n\n<!-- chunk: 正文 -->\n内容"),
            None,
        );
        insert_doc(
            tmp.path(),
            "c2",
            "compiled",
            Some("# 标题\n\n<!-- chunk: 另一段 -->\n内容二"),
            None,
        );
        let model = crate::embedding::tests::FakeEmbedding { dimension: 4 };
        let events = CapturedEvents::default();

        let summary = run_batch(tmp.path(), &events, &[], STEP_INDEX, None, Some((&model, "fake"))).unwrap();

        assert_eq!(summary.kind, "index");
        assert_eq!(summary.total, 2);
        assert_eq!(summary.succeeded, 2);
        assert_eq!(summary.failed, 0);

        let lines = events.lines();
        assert!(lines[0].starts_with("start:") && lines[0].ends_with(":index:2"));
        assert!(lines[1].contains(":c1:index:true:-"));
        assert!(lines[2].contains(":c2:index:true:-"));
        assert!(lines[3].starts_with("done:") && lines[3].ends_with(":2:0"));

        // jobs 记录已完成
        let jobs = jobs::list_jobs(tmp.path(), 10).unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].status, "completed");
        assert_eq!(jobs[0].succeeded, 2);
        assert_eq!(jobs[0].id, summary.job_id);
    }

    #[test]
    fn test_run_batch_all_compiles_then_indexes_and_records_failure() {
        let tmp = setup();
        insert_doc(tmp.path(), "raw1", "raw", None, None);
        let events = CapturedEvents::default();

        let summary = run_batch(
            tmp.path(),
            &events,
            &[],
            STEP_ALL,
            Some(&OkClient),
            Some((&crate::embedding::tests::FakeEmbedding { dimension: 4 }, "fake")),
        )
        .unwrap();

        assert_eq!(summary.succeeded, 1);
        assert_eq!(summary.failed, 0);
        let lines = events.lines();
        // all = 先 compile 后 index 两个 item 事件
        assert!(lines[1].contains(":raw1:compile:true:-"));
        assert!(lines[2].contains(":raw1:index:true:-"));

        let jobs = jobs::list_jobs(tmp.path(), 10).unwrap();
        assert_eq!(jobs[0].kind, "all");
        assert_eq!(jobs[0].succeeded, 1);
    }

    #[test]
    fn test_run_batch_compile_failure_is_reported_not_fatal() {
        let tmp = setup();
        insert_doc(tmp.path(), "raw1", "raw", None, None);
        insert_doc(tmp.path(), "raw2", "raw", None, None);
        let events = CapturedEvents::default();

        let summary = run_batch(tmp.path(), &events, &[], STEP_COMPILE, Some(&FailingClient), None).unwrap();

        assert_eq!(summary.succeeded, 0);
        assert_eq!(summary.failed, 2);
        assert!(events.lines()[1].contains(":raw1:compile:false:LLM 炸了"));

        let jobs = jobs::list_jobs(tmp.path(), 10).unwrap();
        assert_eq!(jobs[0].failed, 2);
    }

    #[test]
    fn test_run_batch_without_client_reports_unconfigured() {
        let tmp = setup();
        insert_doc(tmp.path(), "raw1", "raw", None, None);
        let events = CapturedEvents::default();

        let summary = run_batch(tmp.path(), &events, &[], STEP_COMPILE, None, None).unwrap();

        assert_eq!(summary.failed, 1);
        assert!(events.lines()[1].contains(":raw1:compile:false:LLM 未配置"));
    }
}
