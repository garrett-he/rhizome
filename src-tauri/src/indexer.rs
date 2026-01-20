//! 索引服务（移植 deprecated core/indexer.py）。
//!
//! 对 status='compiled' 的文档：分块（indexing::split_markdown）→ 批量向量化
//! （EmbeddingModel）→ 事务内替换 chunks/chunks_vec/chunks_fts → 状态流转
//! compiled → indexed。失败写 last_error、状态不前进、不中断批次。空 id 列表
//! 选择全部 compiled；显式 id 可重索引（先删旧 chunks）。raw 文档报错。

use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::documents;
use crate::embedding::EmbeddingModel;
use crate::index_db;
use crate::indexing::split_markdown;

/// 单个文档的索引结果。
#[derive(Serialize, Debug, PartialEq, Clone)]
pub struct IndexOutcome {
    pub document_id: String,
    pub title: Option<String>,
    pub chunks: u32,
    pub ok: bool,
    pub error: Option<String>,
}

/// 索引切片摘要（详情页切片列表）。
#[derive(Serialize, Debug, PartialEq, Clone)]
pub struct ChunkInfo {
    pub id: i64,
    pub label: String,
    pub text: String,
}

/// 索引文档：空 id 列表选择全部 compiled；显式 id 不存在则产出错误结果。
pub fn index_documents(
    vault: &Path,
    document_ids: &[String],
    model: &dyn EmbeddingModel,
    model_name: &str,
) -> Result<Vec<IndexOutcome>, String> {
    let mut conn = index_db::open(vault)?;
    documents::ensure_table(&conn)?;
    index_db::create_vector_schema(&conn, model.dimension())?;
    index_db::ensure_fts_schema(&conn)?;

    let mut outcomes: Vec<IndexOutcome> = Vec::new();
    let tx = conn.transaction().map_err(|e| format!("开启事务失败: {e}"))?;

    if document_ids.is_empty() {
        let mut stmt = tx
            .prepare("SELECT id FROM documents WHERE status = 'compiled' ORDER BY created_at")
            .map_err(|e| format!("查询失败: {e}"))?;
        let ids = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| format!("查询失败: {e}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("读取结果失败: {e}"))?;
        drop(stmt);
        for id in ids {
            outcomes.push(index_one(&tx, model, model_name, &id));
        }
    } else {
        for requested in document_ids {
            let resolved: Option<String> = tx
                .query_row("SELECT id FROM documents WHERE id = ?1", [requested], |row| row.get(0))
                .optional()
                .map_err(|e| format!("查询失败: {e}"))?;
            match resolved {
                Some(id) => outcomes.push(index_one(&tx, model, model_name, &id)),
                None => outcomes.push(IndexOutcome {
                    document_id: requested.clone(),
                    title: None,
                    chunks: 0,
                    ok: false,
                    error: Some("document not found".to_string()),
                }),
            }
        }
    }

    tx.commit().map_err(|e| format!("提交失败: {e}"))?;
    Ok(outcomes)
}

/// 分块、向量化并持久化单个文档；返回结果。
fn index_one(tx: &rusqlite::Transaction, model: &dyn EmbeddingModel, model_name: &str, id: &str) -> IndexOutcome {
    let title = metadata_title(tx, id);
    let row: Option<(String, Option<String>)> = tx
        .query_row(
            "SELECT status, content_compiled FROM documents WHERE id = ?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .unwrap_or(None);
    let Some((status, compiled)) = row else {
        return IndexOutcome {
            document_id: id.to_string(),
            title,
            chunks: 0,
            ok: false,
            error: Some("document not found".to_string()),
        };
    };
    if status == "raw" || compiled.is_none() {
        let message = "index: document has not been compiled; run `compile` first".to_string();
        crate::logging::log("ERROR", &format!("文档索引失败 {id}: {message}"));
        let _ = tx.execute(
            "UPDATE documents SET last_error = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, message, now_rfc3339()],
        );
        return IndexOutcome {
            document_id: id.to_string(),
            title,
            chunks: 0,
            ok: false,
            error: Some(message),
        };
    }

    let chunks = split_markdown(compiled.as_deref().unwrap_or_default());
    if chunks.is_empty() {
        let message = "index: document has no content to index".to_string();
        crate::logging::log("ERROR", &format!("文档索引失败 {id}: {message}"));
        let _ = tx.execute(
            "UPDATE documents SET last_error = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, message, now_rfc3339()],
        );
        return IndexOutcome {
            document_id: id.to_string(),
            title,
            chunks: 0,
            ok: false,
            error: Some(message),
        };
    }

    let texts: Vec<String> = chunks.iter().map(|c| c.text.clone()).collect();
    let vectors = match model.embed(&texts) {
        Ok(vectors) => vectors,
        Err(error) => {
            let message = format!("index: {error}");
            crate::logging::log("ERROR", &format!("文档索引失败 {id}: {message}"));
            let _ = tx.execute(
                "UPDATE documents SET last_error = ?2, updated_at = ?3 WHERE id = ?1",
                params![id, message, now_rfc3339()],
            );
            return IndexOutcome {
                document_id: id.to_string(),
                title,
                chunks: 0,
                ok: false,
                error: Some(error),
            };
        }
    };

    if let Err(error) = index_db::replace_chunks(tx, id, &chunks, &vectors) {
        let message = format!("index: {error}");
        crate::logging::log("ERROR", &format!("文档索引失败 {id}: {message}"));
        let _ = tx.execute(
            "UPDATE documents SET last_error = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, message, now_rfc3339()],
        );
        return IndexOutcome {
            document_id: id.to_string(),
            title,
            chunks: 0,
            ok: false,
            error: Some(error),
        };
    }

    let chunk_count = chunks.len() as u32;
    if let Err(error) = tx.execute(
        "UPDATE documents
         SET status = 'indexed', indexed_at = ?2, chunk_count = ?3, embedding_model = ?4,
             last_error = NULL, updated_at = ?2
         WHERE id = ?1",
        params![id, now_rfc3339(), chunk_count, model_name],
    ) {
        return IndexOutcome {
            document_id: id.to_string(),
            title,
            chunks: chunk_count,
            ok: false,
            error: Some(format!("写入数据库失败: {error}")),
        };
    }
    crate::logging::log(
        "INFO",
        &format!("文档索引完成 {id}: {chunk_count} 个分块（{model_name}）"),
    );
    IndexOutcome {
        document_id: id.to_string(),
        title,
        chunks: chunk_count,
        ok: true,
        error: None,
    }
}

/// 读取已编译元数据中的标题（无元数据为 None）。
fn metadata_title(tx: &rusqlite::Transaction, id: &str) -> Option<String> {
    let raw: Option<String> = tx
        .query_row("SELECT metadata_compiled FROM documents WHERE id = ?1", [id], |row| {
            row.get(0)
        })
        .optional()
        .unwrap_or(None);
    let raw = raw?;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    value
        .get("title")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

/// 读取文档切片（详情页切片列表）；文档不存在返回 Err，无切片返回空列表。
pub fn list_chunks(vault: &Path, document_id: &str) -> Result<Vec<ChunkInfo>, String> {
    let conn = Connection::open(crate::db::db_path(vault)).map_err(|e| format!("打开数据库失败: {e}"))?;
    documents::ensure_table(&conn)?;
    let exists: Option<String> = conn
        .query_row("SELECT id FROM documents WHERE id = ?1", [document_id], |row| {
            row.get(0)
        })
        .optional()
        .map_err(|e| format!("查询失败: {e}"))?;
    if exists.is_none() {
        return Err(format!("文档不存在: {document_id}"));
    }
    let chunks_table_exists: bool = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'chunks'",
            [],
            |_| Ok(()),
        )
        .optional()
        .map_err(|e| format!("检查 chunks 表失败: {e}"))?
        .is_some();
    if !chunks_table_exists {
        return Ok(Vec::new());
    }
    let mut stmt = conn
        .prepare("SELECT id, label, text FROM chunks WHERE document_id = ?1 ORDER BY id")
        .map_err(|e| format!("查询失败: {e}"))?;
    let rows = stmt
        .query_map([document_id], |row| {
            Ok(ChunkInfo {
                id: row.get(0)?,
                label: row.get(1)?,
                text: row.get(2)?,
            })
        })
        .map_err(|e| format!("查询失败: {e}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("读取结果失败: {e}"))
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
    use tempfile::TempDir;

    use crate::embedding::tests::FakeEmbedding;

    fn setup() -> TempDir {
        let tmp = TempDir::new().unwrap();
        ensure_db(tmp.path()).unwrap();
        tmp
    }

    /// 插入文档并可选地带编译产物。
    fn insert_doc(tmp: &TempDir, id: &str, status: &str, compiled: Option<&str>) {
        let conn = Connection::open(crate::db::db_path(tmp.path())).unwrap();
        let metadata = compiled.map(|_| r#"{"title": "测试标题"}"#);
        conn.execute(
            "INSERT INTO documents (id, file_name, mime_type, type, sha1, size, content, status, created_at, updated_at, content_compiled, metadata_compiled)
             VALUES (?1, ?1, 'text/markdown', 'plaintext', ?1, 1, 'x', ?2, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', ?3, ?4)",
            params![id, status, compiled, metadata],
        )
        .unwrap();
    }

    fn doc_row(tmp: &TempDir, id: &str) -> (String, Option<String>, Option<i64>, Option<String>, Option<String>) {
        let conn = Connection::open(crate::db::db_path(tmp.path())).unwrap();
        conn.query_row(
            "SELECT status, last_error, chunk_count, embedding_model, indexed_at FROM documents WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .unwrap()
    }

    const COMPILED_BODY: &str = "# 标题\n\n<!-- chunk: 正文 -->\n第一段内容。\n\n<!-- chunk: 结尾 -->\n最后一段。";

    #[test]
    fn test_index_compiled_document() {
        let tmp = setup();
        insert_doc(&tmp, "a", "compiled", Some(COMPILED_BODY));
        let model = FakeEmbedding { dimension: 4 };

        let outcomes = index_documents(tmp.path(), &["a".to_string()], &model, "fake-embedding").unwrap();

        assert!(outcomes[0].ok);
        assert_eq!(outcomes[0].chunks, 2);
        assert_eq!(outcomes[0].title.as_deref(), Some("测试标题"));
        let (status, last_error, chunk_count, embedding_model, indexed_at) = doc_row(&tmp, "a");
        assert_eq!(status, "indexed");
        assert_eq!(last_error, None);
        assert_eq!(chunk_count, Some(2));
        assert_eq!(embedding_model.as_deref(), Some("fake-embedding"));
        assert!(indexed_at.is_some());

        let conn = Connection::open(crate::db::db_path(tmp.path())).unwrap();
        let (chunks, vecs, fts): (i64, i64, i64) = conn
            .query_row(
                "SELECT (SELECT COUNT(*) FROM chunks), (SELECT COUNT(*) FROM chunks_vec), (SELECT COUNT(*) FROM chunks_fts)",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!((chunks, vecs, fts), (2, 2, 2));
    }

    #[test]
    fn test_index_raw_document_is_rejected() {
        let tmp = setup();
        insert_doc(&tmp, "a", "raw", None);
        let model = FakeEmbedding { dimension: 4 };

        let outcomes = index_documents(tmp.path(), &["a".to_string()], &model, "fake-embedding").unwrap();

        assert!(!outcomes[0].ok);
        assert!(outcomes[0]
            .error
            .as_deref()
            .is_some_and(|e| e.contains("has not been compiled")));
        let (status, last_error, _, _, _) = doc_row(&tmp, "a");
        assert_eq!(status, "raw");
        assert!(last_error.is_some_and(|e| e.contains("has not been compiled")));
    }

    #[test]
    fn test_index_empty_body_is_error() {
        let tmp = setup();
        insert_doc(&tmp, "a", "compiled", Some("   "));
        let model = FakeEmbedding { dimension: 4 };

        let outcomes = index_documents(tmp.path(), &["a".to_string()], &model, "fake-embedding").unwrap();

        assert!(!outcomes[0].ok);
        assert!(outcomes[0]
            .error
            .as_deref()
            .is_some_and(|e| e.contains("no content to index")));
    }

    #[test]
    fn test_index_missing_id_is_error_outcome() {
        let tmp = setup();
        let model = FakeEmbedding { dimension: 4 };

        let outcomes = index_documents(tmp.path(), &["nope".to_string()], &model, "fake-embedding").unwrap();

        assert!(!outcomes[0].ok);
        assert_eq!(outcomes[0].error.as_deref(), Some("document not found"));
    }

    #[test]
    fn test_index_empty_ids_selects_compiled_only() {
        let tmp = setup();
        insert_doc(&tmp, "raw1", "raw", None);
        insert_doc(&tmp, "c1", "compiled", Some(COMPILED_BODY));
        insert_doc(&tmp, "c2", "compiled", Some(COMPILED_BODY));
        let model = FakeEmbedding { dimension: 4 };

        let outcomes = index_documents(tmp.path(), &[], &model, "fake-embedding").unwrap();

        assert_eq!(outcomes.len(), 2);
        assert!(outcomes.iter().all(|o| o.ok));
    }

    #[test]
    fn test_reindex_replaces_old_chunks() {
        let tmp = setup();
        insert_doc(&tmp, "a", "compiled", Some(COMPILED_BODY));
        let model = FakeEmbedding { dimension: 4 };
        index_documents(tmp.path(), &["a".to_string()], &model, "fake-embedding").unwrap();

        // 重索引：同样的内容，chunk 数不变但表不膨胀
        let outcomes = index_documents(tmp.path(), &["a".to_string()], &model, "fake-embedding").unwrap();
        assert!(outcomes[0].ok);
        let conn = Connection::open(crate::db::db_path(tmp.path())).unwrap();
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM chunks", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_embed_failure_writes_last_error() {
        let tmp = setup();
        insert_doc(&tmp, "a", "compiled", Some(COMPILED_BODY));
        let model = FailingEmbedding;

        let outcomes = index_documents(tmp.path(), &["a".to_string()], &model, "fake-embedding").unwrap();

        assert!(!outcomes[0].ok);
        assert_eq!(outcomes[0].error.as_deref(), Some("模型炸了"));
        let (status, last_error, _, _, _) = doc_row(&tmp, "a");
        assert_eq!(status, "compiled");
        assert_eq!(last_error.as_deref(), Some("index: 模型炸了"));
    }

    /// 恒定失败的 fake embedding。
    struct FailingEmbedding;

    impl EmbeddingModel for FailingEmbedding {
        fn dimension(&self) -> usize {
            4
        }

        fn embed(&self, _texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
            Err("模型炸了".to_string())
        }
    }

    #[test]
    fn test_list_chunks_ordered_and_empty_cases() {
        let tmp = setup();
        insert_doc(&tmp, "a", "compiled", Some(COMPILED_BODY));
        insert_doc(&tmp, "raw1", "raw", None);
        let model = FakeEmbedding { dimension: 4 };
        index_documents(tmp.path(), &["a".to_string()], &model, "fake").unwrap();

        let chunks = list_chunks(tmp.path(), "a").unwrap();
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].label, "正文");
        assert!(chunks[0].id < chunks[1].id);
        assert!(chunks[1].text.contains("最后一段"));

        // raw 文档无切片 → 空列表
        assert!(list_chunks(tmp.path(), "raw1").unwrap().is_empty());
        // 文档不存在 → Err
        assert!(list_chunks(tmp.path(), "nope").unwrap_err().contains("文档不存在"));
    }
}
