//! 问答（05 问答）：会话持久化、检索上下文组装与流式回答编排。
//!
//! 流程：插入 user 消息 → 检索 top-5（复用 search.rs，向量腿降级语义自动继承）
//! → docs_answer 提示词组装（settings KV 覆盖 → 内置默认）→ `chat_stream`
//! 逐 delta 经 `QaEvents` 回调推送 → 回答与引用落库。事件由命令层的
//! AppHandle 实现推送到前端（qa-start/qa-delta/qa-done/qa-error）。

use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use uuid::Uuid;

use crate::embedding::EmbeddingModel;
use crate::llm::StreamLlmClient;
use crate::search;

/// 检索上下文条数（注入提示词的命中上限）。
pub const CONTEXT_HITS: usize = 5;
/// 会话标题取首问的最大字符数。
const TITLE_MAX_CHARS: usize = 20;

pub const CONVERSATIONS_TABLE_SQL: &str = "CREATE TABLE IF NOT EXISTS conversations (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);";

pub const MESSAGES_TABLE_SQL: &str = "CREATE TABLE IF NOT EXISTS conversation_messages (
    id INTEGER PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    role TEXT NOT NULL CHECK(role IN ('user','assistant')),
    content TEXT NOT NULL,
    sources TEXT,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_conv_messages ON conversation_messages(conversation_id, id);";

/// 引用来源（回答下方的可点击 chip）。
#[derive(Serialize, serde::Deserialize, Debug, PartialEq, Clone)]
pub struct SourceRef {
    pub chunk_id: i64,
    pub document_id: String,
    pub title: String,
    pub chunk_index: u32,
}

/// 历史会话列表项。
#[derive(Serialize, Debug, PartialEq, Clone)]
pub struct ConversationSummary {
    pub id: String,
    pub title: String,
    pub updated_at: String,
}

/// 会话消息（assistant 消息带 sources）。
#[derive(Serialize, Debug, PartialEq, Clone)]
pub struct Message {
    pub id: i64,
    pub role: String,
    pub content: String,
    pub sources: Vec<SourceRef>,
    pub created_at: String,
}

/// 一次提问的最终结果。
#[derive(Serialize, Debug, PartialEq, Clone)]
pub struct AskOutcome {
    pub conversation_id: String,
    pub sources: Vec<SourceRef>,
}

/// 事件回调（命令层用 AppHandle 实现，测试用捕获器）。
pub trait QaEvents: Send + Sync {
    fn start(&self, conversation_id: &str);
    fn delta(&self, conversation_id: &str, text: &str);
    fn done(&self, conversation_id: &str, sources: &[SourceRef], message_id: i64);
    fn error(&self, conversation_id: &str, message: &str);
}

/// 确保会话两张表存在（幂等）。
pub fn ensure_table(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(&format!("{CONVERSATIONS_TABLE_SQL}\n{MESSAGES_TABLE_SQL}"))
        .map_err(|e| format!("初始化会话表失败: {e}"))
}

/// 打开库连接并保证会话表存在。
pub fn open_conn(vault: &Path) -> Result<Connection, String> {
    let conn = crate::db::open_conn(vault)?;
    ensure_table(&conn)?;
    Ok(conn)
}

/// 问答主流程：见模块注释。LLM 流式中途失败时已收 delta 保留、user 消息已落库。
pub fn qa_ask_service(
    vault: &Path,
    events: &dyn QaEvents,
    question: &str,
    conversation_id: Option<&str>,
    client: &dyn StreamLlmClient,
    model: Option<&dyn EmbeddingModel>,
) -> Result<AskOutcome, String> {
    let question = question.trim();
    if question.is_empty() {
        return Err("问题不能为空".to_string());
    }

    let conn = open_conn(vault)?;
    let conversation_id = resolve_conversation(&conn, conversation_id, question)?;

    insert_message(&conn, &conversation_id, "user", question, None)?;
    touch_conversation(&conn, &conversation_id)?;

    let hits = search::search_documents(vault, question, CONTEXT_HITS, model)?;
    let sources: Vec<SourceRef> = hits
        .iter()
        .map(|hit| SourceRef {
            chunk_id: hit.chunk_id,
            document_id: hit.document_id.clone(),
            title: hit.document_title.clone(),
            chunk_index: hit.chunk_index,
        })
        .collect();
    let prompt = resolve_prompt(vault)?;
    let prompt = prompt
        .replace("{context}", &format_context(&hits))
        .replace("{question}", question);

    events.start(&conversation_id);
    let answer = match client.chat_stream(&prompt, &mut |delta| events.delta(&conversation_id, delta)) {
        Ok(answer) => answer,
        Err(error) => {
            events.error(&conversation_id, &error);
            return Err(error);
        }
    };

    let sources_json = serde_json::to_string(&sources).map_err(|e| format!("序列化引用失败: {e}"))?;
    let message_id = insert_message(&conn, &conversation_id, "assistant", &answer, Some(&sources_json))?;
    touch_conversation(&conn, &conversation_id)?;
    events.done(&conversation_id, &sources, message_id);
    Ok(AskOutcome {
        conversation_id,
        sources,
    })
}

/// 历史会话列表（按最近更新降序）。
pub fn list_conversations(vault: &Path) -> Result<Vec<ConversationSummary>, String> {
    let conn = open_conn(vault)?;
    let mut stmt = conn
        .prepare("SELECT id, title, updated_at FROM conversations ORDER BY updated_at DESC")
        .map_err(|e| format!("查询失败: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(ConversationSummary {
                id: row.get(0)?,
                title: row.get(1)?,
                updated_at: row.get(2)?,
            })
        })
        .map_err(|e| format!("查询失败: {e}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("读取结果失败: {e}"))
}

/// 会话消息列表（按 id 升序）；会话不存在返回 Err。
pub fn list_messages(vault: &Path, conversation_id: &str) -> Result<Vec<Message>, String> {
    let conn = open_conn(vault)?;
    let exists: Option<String> = conn
        .query_row("SELECT id FROM conversations WHERE id = ?1", [conversation_id], |row| {
            row.get(0)
        })
        .optional()
        .map_err(|e| format!("查询失败: {e}"))?;
    if exists.is_none() {
        return Err(format!("会话不存在: {conversation_id}"));
    }
    let mut stmt = conn
        .prepare("SELECT id, role, content, sources, created_at FROM conversation_messages WHERE conversation_id = ?1 ORDER BY id")
        .map_err(|e| format!("查询失败: {e}"))?;
    let rows = stmt
        .query_map([conversation_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .map_err(|e| format!("查询失败: {e}"))?;
    let mut messages = Vec::new();
    for row in rows {
        let (id, role, content, sources_raw, created_at) = row.map_err(|e| format!("读取结果失败: {e}"))?;
        messages.push(Message {
            id,
            role,
            content,
            sources: parse_sources(sources_raw.as_deref()),
            created_at,
        });
    }
    Ok(messages)
}

/// 解析会话 id：None 新建（标题取首问前 20 字）；传入但不存在则 Err。
fn resolve_conversation(conn: &Connection, conversation_id: Option<&str>, question: &str) -> Result<String, String> {
    if let Some(id) = conversation_id {
        let exists: Option<String> = conn
            .query_row("SELECT id FROM conversations WHERE id = ?1", [id], |row| row.get(0))
            .optional()
            .map_err(|e| format!("查询失败: {e}"))?;
        return exists.ok_or_else(|| format!("会话不存在: {id}"));
    }
    let id = Uuid::new_v4().to_string();
    let title = truncate_chars(question, TITLE_MAX_CHARS);
    let now = now_rfc3339();
    conn.execute(
        "INSERT INTO conversations (id, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
        params![id, title, now],
    )
    .map_err(|e| format!("创建会话失败: {e}"))?;
    Ok(id)
}

fn insert_message(
    conn: &Connection,
    conversation_id: &str,
    role: &str,
    content: &str,
    sources_json: Option<&str>,
) -> Result<i64, String> {
    conn.execute(
        "INSERT INTO conversation_messages (conversation_id, role, content, sources, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![conversation_id, role, content, sources_json, now_rfc3339()],
    )
    .map_err(|e| format!("写入消息失败: {e}"))?;
    Ok(conn.last_insert_rowid())
}

fn touch_conversation(conn: &Connection, conversation_id: &str) -> Result<(), String> {
    conn.execute(
        "UPDATE conversations SET updated_at = ?2 WHERE id = ?1",
        params![conversation_id, now_rfc3339()],
    )
    .map_err(|e| format!("更新会话失败: {e}"))?;
    Ok(())
}

/// 提示词：settings KV prompt:docs_answer 覆盖 → 内置默认。
fn resolve_prompt(vault: &Path) -> Result<String, String> {
    let custom = crate::settings::get(vault, "prompt:docs_answer")?.filter(|v| !v.trim().is_empty());
    let template = match custom {
        Some(custom) => custom,
        None => crate::settings::default_prompt(crate::settings::PROMPT_DOCS_ANSWER)
            .ok_or_else(|| "内置问答提示词缺失".to_string())?
            .to_string(),
    };
    Ok(template)
}

/// 上下文块：每条「①标题（chunk 02）：文本」。
fn format_context(hits: &[search::SearchHit]) -> String {
    hits.iter()
        .map(|hit| {
            let marker = circled_number(hit_index(hits, hit));
            format!(
                "{marker} {}（chunk {:02}）：{}",
                hit.document_title, hit.chunk_index, hit.text
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn hit_index(hits: &[search::SearchHit], hit: &search::SearchHit) -> usize {
    hits.iter().position(|h| h.chunk_id == hit.chunk_id).unwrap_or(0)
}

/// 1..=20 的带圈数字（超出回退 (n)）。
fn circled_number(index: usize) -> String {
    const CIRCLED: [char; 20] = [
        '①', '②', '③', '④', '⑤', '⑥', '⑦', '⑧', '⑨', '⑩', '⑪', '⑫', '⑬', '⑭', '⑮', '⑯', '⑰', '⑱', '⑲', '⑳',
    ];
    match CIRCLED.get(index) {
        Some(marker) => marker.to_string(),
        None => format!("({})", index + 1),
    }
}

fn parse_sources(raw: Option<&str>) -> Vec<SourceRef> {
    let Some(raw) = raw else {
        return Vec::new();
    };
    serde_json::from_str(raw).unwrap_or_default()
}

fn truncate_chars(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    text.chars().take(max).collect()
}

fn now_rfc3339() -> String {
    use time::format_description::well_known::Rfc3339;
    use time::OffsetDateTime;
    OffsetDateTime::now_utc().format(&Rfc3339).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::ensure_db;
    use crate::embedding::tests::FakeEmbedding;
    use crate::indexer;
    use tempfile::TempDir;

    fn setup() -> TempDir {
        let tmp = TempDir::new().unwrap();
        ensure_db(tmp.path()).unwrap();
        tmp
    }

    const BODY_A: &str = "<!-- chunk: 服务拆分 -->\n拆分服务时要识别业务边界。";

    fn index_fixture(tmp: &TempDir) {
        let conn = Connection::open(crate::db::db_path(tmp.path())).unwrap();
        conn.execute(
            "INSERT INTO documents (id, file_name, mime_type, type, sha1, size, content, status, created_at, updated_at, content_compiled, metadata_compiled)
             VALUES ('a', 'a.md', 'text/markdown', 'plaintext', 'a', 1, 'x', 'compiled', '2026-10-20T01:00:00Z', '2026-10-20T01:00:00Z', ?1, ?2)",
            params![BODY_A, r#"{"title": "微服务架构实践", "tags": ["技术"]}"#],
        )
        .unwrap();
        let model = FakeEmbedding { dimension: 4 };
        indexer::index_documents(tmp.path(), &["a".to_string()], &model, "fake").unwrap();
    }

    /// 确定性流式 fake：按预设 delta 序列回调并记录收到的 prompt。
    struct FakeStreamClient {
        deltas: Vec<String>,
        prompts: std::sync::Mutex<Vec<String>>,
        fail: bool,
    }

    impl FakeStreamClient {
        fn new(deltas: &[&str]) -> Self {
            Self {
                deltas: deltas.iter().map(|s| s.to_string()).collect(),
                prompts: std::sync::Mutex::new(Vec::new()),
                fail: false,
            }
        }

        fn failing() -> Self {
            Self {
                deltas: Vec::new(),
                prompts: std::sync::Mutex::new(Vec::new()),
                fail: true,
            }
        }

        fn last_prompt(&self) -> String {
            self.prompts.lock().unwrap().last().cloned().unwrap_or_default()
        }
    }

    impl StreamLlmClient for FakeStreamClient {
        fn chat_stream(&self, prompt: &str, on_delta: &mut dyn FnMut(&str)) -> Result<String, String> {
            self.prompts.lock().unwrap().push(prompt.to_string());
            if self.fail {
                return Err("模型炸了".to_string());
            }
            for delta in &self.deltas {
                on_delta(delta);
            }
            Ok(self.deltas.concat())
        }
    }

    /// 事件捕获器：记录事件序列。
    #[derive(Default)]
    struct CapturedEvents {
        log: std::sync::Mutex<Vec<String>>,
    }

    impl CapturedEvents {
        fn push(&self, event: &str) {
            self.log.lock().unwrap().push(event.to_string());
        }
    }

    impl QaEvents for CapturedEvents {
        fn start(&self, conversation_id: &str) {
            self.push(&format!("start:{conversation_id}"));
        }

        fn delta(&self, conversation_id: &str, text: &str) {
            self.push(&format!("delta:{conversation_id}:{text}"));
        }

        fn done(&self, conversation_id: &str, sources: &[SourceRef], message_id: i64) {
            self.push(&format!("done:{conversation_id}:{}:{message_id}", sources.len()));
        }

        fn error(&self, conversation_id: &str, message: &str) {
            self.push(&format!("error:{conversation_id}:{message}"));
        }
    }

    #[test]
    fn test_ask_creates_conversation_and_streams() {
        let tmp = setup();
        index_fixture(&tmp);
        let events = CapturedEvents::default();
        let client = FakeStreamClient::new(&["服务拆", "分要点"]);
        let model = FakeEmbedding { dimension: 4 };

        let outcome = qa_ask_service(tmp.path(), &events, "服务拆分要注意什么？", None, &client, Some(&model)).unwrap();

        // 事件序列：start → delta*2 → done
        let log = events.log.lock().unwrap();
        assert_eq!(log[0], format!("start:{}", outcome.conversation_id));
        assert_eq!(log[1], format!("delta:{}:服务拆", outcome.conversation_id));
        assert_eq!(log[2], format!("delta:{}:分要点", outcome.conversation_id));
        assert!(log[3].starts_with(&format!("done:{}:1:", outcome.conversation_id)));

        // 标题 = 首问前 20 字
        let conversations = list_conversations(tmp.path()).unwrap();
        assert_eq!(conversations.len(), 1);
        assert_eq!(conversations[0].title, "服务拆分要注意什么？");

        // 消息：user + assistant（带 sources）
        let messages = list_messages(tmp.path(), &outcome.conversation_id).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[1].content, "服务拆分要点");
        assert_eq!(messages[1].sources.len(), 1);
        assert_eq!(messages[1].sources[0].title, "微服务架构实践");
        assert_eq!(messages[1].sources[0].chunk_index, 1);

        // prompt 组装：上下文与问题注入，占位符被替换
        let prompt = client.last_prompt();
        assert!(prompt.contains("① 微服务架构实践（chunk 01）：拆分服务时要识别业务边界。"));
        assert!(prompt.contains("服务拆分要注意什么？"));
        assert!(!prompt.contains("{context}"));
        assert!(!prompt.contains("{question}"));
    }

    #[test]
    fn test_ask_reuses_existing_conversation() {
        let tmp = setup();
        index_fixture(&tmp);
        let model = FakeEmbedding { dimension: 4 };
        let first = qa_ask_service(
            tmp.path(),
            &CapturedEvents::default(),
            "第一问",
            None,
            &FakeStreamClient::new(&["好"]),
            Some(&model),
        )
        .unwrap();
        let second = qa_ask_service(
            tmp.path(),
            &CapturedEvents::default(),
            "第二问",
            Some(&first.conversation_id),
            &FakeStreamClient::new(&["也好"]),
            Some(&model),
        )
        .unwrap();

        assert_eq!(second.conversation_id, first.conversation_id);
        let messages = list_messages(tmp.path(), &first.conversation_id).unwrap();
        assert_eq!(messages.len(), 4);
        // 列表按更新时间降序且仍只有一个会话
        assert_eq!(list_conversations(tmp.path()).unwrap().len(), 1);
    }

    #[test]
    fn test_ask_unknown_conversation_is_error() {
        let tmp = setup();
        let error = qa_ask_service(
            tmp.path(),
            &CapturedEvents::default(),
            "问",
            Some("nope"),
            &FakeStreamClient::new(&[]),
            None,
        )
        .unwrap_err();
        assert!(error.contains("会话不存在"));
    }

    #[test]
    fn test_ask_empty_question_is_error() {
        let tmp = setup();
        assert!(qa_ask_service(
            tmp.path(),
            &CapturedEvents::default(),
            "   ",
            None,
            &FakeStreamClient::new(&[]),
            None
        )
        .is_err());
    }

    #[test]
    fn test_ask_llm_failure_emits_error_and_keeps_user_message() {
        let tmp = setup();
        index_fixture(&tmp);
        let events = CapturedEvents::default();
        let model = FakeEmbedding { dimension: 4 };

        let error = qa_ask_service(
            tmp.path(),
            &events,
            "服务拆分？",
            None,
            &FakeStreamClient::failing(),
            Some(&model),
        )
        .unwrap_err();
        assert_eq!(error, "模型炸了");

        let log = events.log.lock().unwrap();
        assert!(log.iter().any(|e| e.starts_with("start:")));
        assert!(log.iter().any(|e| e.ends_with(":模型炸了")));
        // user 消息已落库，无 assistant 消息
        let id = &list_conversations(tmp.path()).unwrap()[0].id;
        let messages = list_messages(tmp.path(), id).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "user");
    }

    #[test]
    fn test_ask_without_index_still_answers() {
        let tmp = setup();
        let events = CapturedEvents::default();
        let client = FakeStreamClient::new(&["无法回答"]);

        let outcome = qa_ask_service(tmp.path(), &events, "完全无关的问题", None, &client, None).unwrap();

        assert!(outcome.sources.is_empty());
        let prompt = client.last_prompt();
        assert!(prompt.contains("完全无关的问题"));
    }

    #[test]
    fn test_ask_uses_custom_prompt_override() {
        let tmp = setup();
        crate::settings::set(
            tmp.path(),
            "prompt:docs_answer",
            Some("定制提示 {question}".to_string()),
        )
        .unwrap();
        let client = FakeStreamClient::new(&["好"]);

        qa_ask_service(tmp.path(), &CapturedEvents::default(), "问题", None, &client, None).unwrap();

        assert_eq!(client.last_prompt(), "定制提示 问题");
    }

    #[test]
    fn test_list_messages_unknown_conversation_is_error() {
        let tmp = setup();
        assert!(list_messages(tmp.path(), "nope").unwrap_err().contains("会话不存在"));
    }
}
