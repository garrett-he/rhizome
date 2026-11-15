//! 混合检索：向量 KNN + FTS5 trigram 双路 RRF 融合（移植 deprecated
//! mcp/knowledge_base.py 的 search 实现）。
//!
//! 向量腿失败（模型未就绪/维度不匹配/SQL 异常）时降级仅 FTS 并写 app.log，
//! 保证检索始终可用；结果里的 `vector_used` 如实标记向量腿是否生效，前端
//! 据此展示降级状态。相关度为批次内归一化值（top 恒 1.00）。

use std::collections::HashMap;
use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::documents;
use crate::embedding::EmbeddingModel;
use crate::index_db;

/// RRF 融合常数（对齐旧版）。
pub const RRF_K: f64 = 60.0;
/// trigram 分词器无法匹配少于 3 字符的查询。
pub const MIN_FTS_QUERY_CHARS: usize = 3;
const SNIPPET_MAX_CHARS: usize = 200;

/// 一次检索的结果：命中列表 + 向量腿是否实际参与融合。
#[derive(Serialize, Debug, PartialEq, Clone)]
pub struct SearchOutcome {
    pub hits: Vec<SearchHit>,
    /// false 表示本次仅使用全文检索（向量腿不可用），结果可能不完整。
    pub vector_used: bool,
}

/// 单条检索结果。
#[derive(Serialize, Debug, PartialEq, Clone)]
pub struct SearchHit {
    pub chunk_id: i64,
    pub document_id: String,
    /// 文档标题（metadata_compiled.title，回退 file_name）
    pub document_title: String,
    pub chunk_label: String,
    /// chunk 在所属文档中的位次（从 1 起）
    pub chunk_index: u32,
    /// chunk 文本截断片段（约 200 字符 + 省略号）
    pub snippet: String,
    /// chunk 完整文本（问答上下文注入用；列表页可忽略）
    pub text: String,
    pub tags: Vec<String>,
    pub created_at: String,
    /// 批次内归一化相关度（0..1，top = 1.00）
    pub relevance: f64,
}

/// 混合检索入口：空查询返回空结果；无索引库返回空结果（而非报错）。
#[allow(clippy::too_many_lines)]
pub fn search_documents(
    vault: &Path,
    query: &str,
    limit: usize,
    model: Option<&dyn EmbeddingModel>,
) -> Result<SearchOutcome, String> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(SearchOutcome {
            hits: Vec::new(),
            vector_used: false,
        });
    }
    let limit = limit.clamp(1, 50);

    let conn = index_db::open(vault)?;
    documents::ensure_table(&conn)?;
    if !table_exists(&conn, "chunks") {
        return Ok(SearchOutcome {
            hits: Vec::new(),
            vector_used: false,
        });
    }

    let (vector_ids, vector_used) = vector_leg(&conn, query, limit, model);
    let fts_ids = fts_leg(&conn, query, limit);
    if vector_ids.is_empty() && fts_ids.is_empty() {
        return Ok(SearchOutcome {
            hits: Vec::new(),
            vector_used,
        });
    }

    // RRF 融合：score += 1 / (RRF_K + rank)，rank 从 1 起
    let mut scores: HashMap<i64, f64> = HashMap::new();
    for (rank, chunk_id) in vector_ids.iter().enumerate() {
        *scores.entry(*chunk_id).or_default() += 1.0 / (RRF_K + rank as f64 + 1.0);
    }
    for (rank, chunk_id) in fts_ids.iter().enumerate() {
        *scores.entry(*chunk_id).or_default() += 1.0 / (RRF_K + rank as f64 + 1.0);
    }

    let mut ranked: Vec<(i64, f64)> = scores.into_iter().collect();
    // 分数降序；同分按 chunk_id 升序保证确定性
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    ranked.truncate(limit);
    let max_score = ranked.first().map_or(0.0, |(_, score)| *score);

    let mut hits = Vec::with_capacity(ranked.len());
    for (chunk_id, score) in ranked {
        if let Some(mut hit) = load_hit(&conn, chunk_id)? {
            hit.relevance = if max_score > 0.0 { score / max_score } else { 0.0 };
            hits.push(hit);
        }
    }
    Ok(SearchOutcome { hits, vector_used })
}

/// 向量腿：查询文本本地 embedding 后 KNN；返回（chunk id, 向量腿是否生效）。
/// 任何失败降级为空并写 log，第二个元素为 false。
fn vector_leg(conn: &Connection, query: &str, limit: usize, model: Option<&dyn EmbeddingModel>) -> (Vec<i64>, bool) {
    let Some(model) = model else {
        return (Vec::new(), false);
    };
    if !table_exists(conn, "chunks_vec") {
        crate::logging::log("INFO", "向量腿跳过：chunks_vec 未建立（尚未索引）");
        return (Vec::new(), false);
    }
    let stored = match index_db::stored_dimension(conn) {
        Ok(stored) => stored,
        Err(error) => {
            crate::logging::log("ERROR", &format!("向量腿跳过：读取索引维度失败: {error}"));
            return (Vec::new(), false);
        }
    };
    if stored.is_none() {
        crate::logging::log("INFO", "向量腿跳过：索引未记录嵌入维度");
        return (Vec::new(), false);
    }
    if stored != Some(model.dimension()) {
        crate::logging::log(
            "ERROR",
            &format!(
                "向量腿跳过：嵌入维度不匹配（索引 {} 维，模型 {} 维），仅使用全文检索",
                stored.unwrap_or(0),
                model.dimension()
            ),
        );
        return (Vec::new(), false);
    }
    let vector = match model.embed(&[query.to_string()]) {
        Ok(mut vectors) if vectors.len() == 1 => match serde_json::to_string(&vectors.remove(0)) {
            Ok(vector) => vector,
            Err(error) => {
                crate::logging::log("ERROR", &format!("向量腿跳过：查询向量序列化失败: {error}"));
                return (Vec::new(), false);
            }
        },
        Ok(_) => {
            crate::logging::log("ERROR", "向量腿跳过：embedding 返回数量异常");
            return (Vec::new(), false);
        }
        Err(error) => {
            crate::logging::log("ERROR", &format!("向量腿跳过：查询向量化失败: {error}"));
            return (Vec::new(), false);
        }
    };
    let mut stmt =
        match conn.prepare("SELECT chunk_id FROM chunks_vec WHERE embedding MATCH ?1 AND k = ?2 ORDER BY distance") {
            Ok(stmt) => stmt,
            Err(error) => {
                crate::logging::log("ERROR", &format!("向量腿跳过：KNN prepare 失败: {error}"));
                return (Vec::new(), false);
            }
        };
    let rows = stmt.query_map(params![vector, limit as i64], |row| row.get::<_, i64>(0));
    match rows {
        Ok(rows) => (rows.filter_map(Result::ok).collect(), true),
        Err(error) => {
            crate::logging::log("ERROR", &format!("向量腿跳过：KNN 查询失败: {error}"));
            (Vec::new(), false)
        }
    }
}

/// 把用户查询转为 FTS5 短语法：按空白分词，每项加双引号并转义内部引号。
/// 避免查询中的 FTS5 操作符（AND/OR/NEAR/*/括号/引号）触发语法错误导致
/// 全文腿静默失效。多词之间用空格连接（隐式 AND）。
fn fts_query(query: &str) -> String {
    query
        .split_whitespace()
        .map(|term| format!("\"{}\"", term.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" ")
}

/// FTS 腿：trigram 查询需 >= 3 字符；MATCH 语法错误降级为空（log 留证据）。
fn fts_leg(conn: &Connection, query: &str, limit: usize) -> Vec<i64> {
    if query.chars().count() < MIN_FTS_QUERY_CHARS {
        return Vec::new();
    }
    if !table_exists(conn, "chunks_fts") {
        return Vec::new();
    }
    let match_query = fts_query(query);
    let mut stmt = match conn.prepare("SELECT rowid FROM chunks_fts WHERE chunks_fts MATCH ?1 ORDER BY rank LIMIT ?2") {
        Ok(stmt) => stmt,
        Err(error) => {
            crate::logging::log("ERROR", &format!("FTS 腿跳过：prepare 失败: {error}"));
            return Vec::new();
        }
    };
    let rows = stmt.query_map(params![match_query, limit as i64], |row| row.get::<_, i64>(0));
    match rows {
        Ok(rows) => rows.filter_map(Result::ok).collect(),
        Err(error) => {
            crate::logging::log("ERROR", &format!("FTS 腿跳过：查询失败: {error}"));
            Vec::new()
        }
    }
}

/// 加载单条命中：chunks JOIN documents + chunk 位次 + 元数据解析。
fn load_hit(conn: &Connection, chunk_id: i64) -> Result<Option<SearchHit>, String> {
    let row = conn
        .query_row(
            "SELECT c.label, c.text, d.id, d.file_name, d.metadata_compiled, d.created_at,
                    (SELECT COUNT(*) FROM chunks c2 WHERE c2.document_id = c.document_id AND c2.id <= c.id)
             FROM chunks c JOIN documents d ON d.id = c.document_id
             WHERE c.id = ?1",
            [chunk_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)?,
                ))
            },
        )
        .optional()
        .map_err(|e| format!("查询命中失败: {e}"))?;
    let Some((label, text, document_id, file_name, metadata_raw, created_at, ordinal)) = row else {
        return Ok(None);
    };
    let metadata = parse_metadata(metadata_raw.as_deref());
    Ok(Some(SearchHit {
        chunk_id,
        document_id,
        document_title: metadata.title.unwrap_or(file_name),
        chunk_label: label,
        chunk_index: ordinal.max(0) as u32,
        snippet: truncate_snippet(&text),
        text,
        tags: metadata.tags,
        created_at,
        relevance: 0.0,
    }))
}

struct ParsedMetadata {
    title: Option<String>,
    tags: Vec<String>,
}

fn parse_metadata(raw: Option<&str>) -> ParsedMetadata {
    let Some(raw) = raw else {
        return ParsedMetadata {
            title: None,
            tags: Vec::new(),
        };
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
        return ParsedMetadata {
            title: None,
            tags: Vec::new(),
        };
    };
    let title = value
        .get("title")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let tags = value
        .get("tags")
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(serde_json::Value::as_str)
                .filter(|tag| !tag.trim().is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    ParsedMetadata { title, tags }
}

/// 截断片段：超过 200 字符时截断并追加中文省略号。
fn truncate_snippet(text: &str) -> String {
    if text.chars().count() <= SNIPPET_MAX_CHARS {
        return text.to_string();
    }
    let truncated: String = text.chars().take(SNIPPET_MAX_CHARS).collect();
    format!("{truncated}……")
}

fn table_exists(conn: &Connection, name: &str) -> bool {
    conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [name],
        |_| Ok(()),
    )
    .optional()
    .unwrap_or(None)
    .is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::ensure_db;
    use crate::embedding::tests::FakeEmbedding;
    use crate::indexer;
    use rusqlite::Connection;
    use tempfile::TempDir;

    /// 恒定失败的 fake embedding（测试降级路径）。
    struct FailingEmbedding;

    impl EmbeddingModel for FailingEmbedding {
        fn dimension(&self) -> usize {
            4
        }

        fn embed(&self, _texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
            Err("模型炸了".to_string())
        }
    }

    fn setup() -> TempDir {
        let tmp = TempDir::new().unwrap();
        ensure_db(tmp.path()).unwrap();
        tmp
    }

    fn insert_doc(tmp: &TempDir, id: &str, compiled: &str, metadata: &str) {
        let conn = Connection::open(crate::db::db_path(tmp.path())).unwrap();
        conn.execute(
            "INSERT INTO documents (id, file_name, mime_type, type, sha1, size, content, status, created_at, updated_at, content_compiled, metadata_compiled)
             VALUES (?1, ?1, 'text/markdown', 'plaintext', ?1, 1, 'x', 'compiled', '2026-10-20T01:00:00Z', '2026-10-20T01:00:00Z', ?2, ?3)",
            params![id, compiled, metadata],
        )
        .unwrap();
    }

    const BODY_A: &str =
        "<!-- chunk: 服务拆分 -->\n拆分服务时要识别业务边界。\n\n<!-- chunk: 数据一致性 -->\n跨服务数据一致性的权衡。";
    const BODY_B: &str = "<!-- chunk: 单体到微服务 -->\n单体应用逐步演进为微服务。";

    /// 建索引（dim 4），文档 a 两个 chunk、文档 b 一个 chunk。
    fn index_fixture(tmp: &TempDir) {
        insert_doc(
            tmp,
            "a",
            BODY_A,
            r#"{"title": "微服务架构实践", "tags": ["技术"], "language": "zh"}"#,
        );
        insert_doc(
            tmp,
            "b",
            BODY_B,
            r#"{"title": "架构演进之路", "tags": ["工作"], "language": "zh"}"#,
        );
        let model = FakeEmbedding { dimension: 4 };
        indexer::index_documents(tmp.path(), &[], &model, "fake").unwrap();
    }

    #[test]
    fn test_empty_query_returns_empty() {
        let tmp = setup();
        let empty = search_documents(tmp.path(), "", 10, None).unwrap();
        assert!(empty.hits.is_empty());
        assert!(!empty.vector_used);
        assert!(search_documents(tmp.path(), "   ", 10, None).unwrap().hits.is_empty());
    }

    #[test]
    fn test_search_without_index_is_empty_not_error() {
        let tmp = setup();
        let outcome = search_documents(tmp.path(), "服务拆分", 10, None).unwrap();
        assert!(outcome.hits.is_empty());
        assert!(!outcome.vector_used);
    }

    #[test]
    fn test_fts_leg_finds_keyword_and_normalizes_relevance() {
        let tmp = setup();
        index_fixture(&tmp);

        let outcome = search_documents(tmp.path(), "业务边界", 10, None).unwrap();
        let hits = outcome.hits;
        // 只有文档 a 的首个 chunk 含「业务边界」
        assert_eq!(hits.len(), 1);
        // top 相关度恒为 1.00
        assert!((hits[0].relevance - 1.0).abs() < 1e-9);
        assert_eq!(hits[0].document_title, "微服务架构实践");
        assert_eq!(hits[0].chunk_index, 1);
        assert_eq!(hits[0].chunk_label, "服务拆分");
        assert_eq!(hits[0].tags, vec!["技术".to_string()]);
        assert!(hits[0].snippet.contains("业务边界"));
    }

    #[test]
    fn test_vector_leg_contributes_when_model_available() {
        let tmp = setup();
        index_fixture(&tmp);
        // "单体应用逐步演进为微服务" 与 chunk b1 文本完全一致 → 语义命中
        let model = FakeEmbedding { dimension: 4 };
        let with_vector = search_documents(tmp.path(), "单体应用逐步演进为微服务", 10, Some(&model)).unwrap();
        let fts_only = search_documents(tmp.path(), "单体应用逐步演进为微服务", 10, None).unwrap();
        assert!(with_vector.vector_used);
        assert!(!fts_only.vector_used);
        assert!(!with_vector.hits.is_empty());
        assert_eq!(with_vector.hits[0].chunk_label, "单体到微服务");
        // 混合腿的 top 分数应 >= 仅 FTS 的 top（双腿命中加分）
        assert!(with_vector.hits[0].relevance >= fts_only.hits[0].relevance);
    }

    /// 回归：embedding 失败时必须标记 vector_used=false，结果集不能伪装成健康的混合检索。
    #[test]
    fn test_degrades_to_fts_when_model_fails() {
        let tmp = setup();
        index_fixture(&tmp);
        let failing = FailingEmbedding;
        let outcome = search_documents(tmp.path(), "业务边界", 10, Some(&failing)).unwrap();
        assert!(!outcome.vector_used, "向量腿失败后必须显式标记降级");
        assert!(!outcome.hits.is_empty());
        assert!((outcome.hits[0].relevance - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_degrades_on_dimension_mismatch() {
        let tmp = setup();
        index_fixture(&tmp);
        // 索引为 4 维，用 2 维模型查询 → 向量腿降级，FTS 出结果
        let small = FakeEmbedding { dimension: 2 };
        let outcome = search_documents(tmp.path(), "业务边界", 10, Some(&small)).unwrap();
        assert!(!outcome.vector_used);
        assert_eq!(outcome.hits.len(), 1);
    }

    #[test]
    fn test_short_query_skips_fts_leg() {
        let tmp = setup();
        index_fixture(&tmp);
        // 2 字符查询 < trigram 最小长度：仅向量腿
        let model = FakeEmbedding { dimension: 4 };
        let outcome = search_documents(tmp.path(), "拆分", 10, Some(&model)).unwrap();
        // "拆分" 两字无法被 trigram FTS 匹配，但向量腿可返回 KNN 结果
        assert!(outcome.vector_used);
        // 关键是不报错
    }

    #[test]
    fn test_limit_clamped_and_results_truncated() {
        let tmp = setup();
        index_fixture(&tmp);
        let hits = search_documents(tmp.path(), "的", 10, None).unwrap().hits;
        assert!(hits.len() <= 10);
        let empty = search_documents(tmp.path(), "业务边界", 0, None).unwrap().hits;
        assert!(empty.is_empty() || empty.len() == 1);
    }

    #[test]
    fn test_snippet_truncates_long_text() {
        let tmp = setup();
        let long_text = "长".repeat(500);
        insert_doc(&tmp, "a", &format!("<!-- chunk: 超长 -->\n{long_text}"), "{}");
        let model = FakeEmbedding { dimension: 4 };
        indexer::index_documents(tmp.path(), std::slice::from_ref(&"a".to_string()), &model, "fake").unwrap();

        let hits = search_documents(tmp.path(), "超长", 10, Some(&model)).unwrap().hits;
        assert_eq!(hits.len(), 1);
        assert!(hits[0].snippet.chars().count() <= SNIPPET_MAX_CHARS + 2);
        assert!(hits[0].snippet.ends_with("……"));
    }
}
