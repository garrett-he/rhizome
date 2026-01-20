//! sqlite-vec / FTS5 向量索引存储（移植 deprecated core/index_db.py）。
//!
//! 向量存于同库的 `chunks_vec` vec0 虚表；`index_meta` 记录嵌入维度，维度
//! 变化（用户切换 embedding_model）时重建虚表并清空 chunks、把已索引文档
//! 状态回退为 compiled（旧版未回退状态，此处补齐）。`chunks_fts` 用 FTS5
//! trigram 分词以支持中文短串匹配，rowid 镜像 chunk id。

use std::path::Path;
use std::sync::Once;

use rusqlite::{params, Connection, OptionalExtension};

use crate::indexing::TextChunk;

pub const INDEX_META_DIMENSION_KEY: &str = "embedding_dimension";

pub const CHUNKS_TABLE_SQL: &str = "CREATE TABLE IF NOT EXISTS chunks (
    id INTEGER PRIMARY KEY,
    document_id TEXT NOT NULL,
    label TEXT NOT NULL DEFAULT '',
    text TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_chunks_document ON chunks(document_id);";

/// 注册 sqlite-vec 静态扩展（进程级一次；对之后打开的连接生效）。
fn register_vec_extension() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        // sqlite-vec 提供与 auto extension 入口签名一致的 init 符号
        type VecInit = unsafe extern "C" fn(
            *mut rusqlite::ffi::sqlite3,
            *mut *mut std::os::raw::c_char,
            *const rusqlite::ffi::sqlite3_api_routines,
        ) -> std::os::raw::c_int;
        unsafe {
            let init: VecInit = std::mem::transmute(sqlite_vec::sqlite3_vec_init as *const ());
            let _ = rusqlite::ffi::sqlite3_auto_extension(Some(init));
        }
    });
}

/// 打开带 sqlite-vec 与 WAL 的库连接。
pub fn open(vault: &Path) -> Result<Connection, String> {
    register_vec_extension();
    let conn = Connection::open(crate::db::db_path(vault)).map_err(|e| format!("打开数据库失败: {e}"))?;
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(|e| format!("设置 WAL 失败: {e}"))?;
    Ok(conn)
}

/// 确保 chunks 基表存在。
pub fn ensure_chunks_table(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(CHUNKS_TABLE_SQL)
        .map_err(|e| format!("初始化 chunks 表失败: {e}"))
}

/// 读取已记录的嵌入维度（未记录为 None）。
fn stored_dimension(conn: &Connection) -> Result<Option<usize>, String> {
    conn.query_row(
        "SELECT value FROM index_meta WHERE key = ?1",
        [INDEX_META_DIMENSION_KEY],
        |row| row.get::<_, String>(0),
    )
    .map(|v| v.parse::<usize>().ok())
    .optional()
    .map(|o| o.flatten())
    .map_err(|e| format!("读取 index_meta 失败: {e}"))
}

/// 创建/校正 vec0 虚表与 FTS 表。维度与已记录值一致时幂等；变化时重建并清空
/// chunks，同时把 indexed 文档状态回退为 compiled（向量已不可比，必须重嵌入）。
pub fn create_vector_schema(conn: &Connection, dimension: usize) -> Result<(), String> {
    ensure_chunks_table(conn)?;
    conn.execute_batch("CREATE TABLE IF NOT EXISTS index_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);")
        .map_err(|e| format!("初始化 index_meta 表失败: {e}"))?;

    let stored = stored_dimension(conn)?;
    if stored == Some(dimension) {
        conn.execute_batch(&format!(
            "CREATE VIRTUAL TABLE IF NOT EXISTS chunks_vec USING vec0(chunk_id INTEGER PRIMARY KEY, embedding float[{dimension}]);"
        ))
        .map_err(|e| format!("初始化向量索引失败: {e}"))?;
        ensure_fts_schema(conn)?;
        return Ok(());
    }

    if stored.is_some() {
        conn.execute_batch("DROP TABLE IF EXISTS chunks_vec; DROP TABLE IF EXISTS chunks_fts; DELETE FROM chunks;")
            .map_err(|e| format!("重建向量索引失败: {e}"))?;
        conn.execute("UPDATE documents SET status = 'compiled' WHERE status = 'indexed'", [])
            .map_err(|e| format!("回退文档状态失败: {e}"))?;
    }
    conn.execute_batch(&format!(
        "CREATE VIRTUAL TABLE chunks_vec USING vec0(chunk_id INTEGER PRIMARY KEY, embedding float[{dimension}]);"
    ))
    .map_err(|e| format!("初始化向量索引失败: {e}"))?;
    conn.execute(
        "INSERT INTO index_meta (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![INDEX_META_DIMENSION_KEY, dimension.to_string()],
    )
    .map_err(|e| format!("记录嵌入维度失败: {e}"))?;
    ensure_fts_schema(conn)
}

/// 确保 chunks_fts 存在并回填既有 chunks。
pub fn ensure_fts_schema(conn: &Connection) -> Result<(), String> {
    let exists: bool = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'chunks_fts'",
            [],
            |_| Ok(()),
        )
        .optional()
        .map_err(|e| format!("检查 chunks_fts 失败: {e}"))?
        .is_some();
    if exists {
        return Ok(());
    }
    conn.execute_batch("CREATE VIRTUAL TABLE chunks_fts USING fts5(text, tokenize='trigram');")
        .map_err(|e| format!("初始化全文索引失败: {e}"))?;
    conn.execute("INSERT INTO chunks_fts(rowid, text) SELECT id, text FROM chunks", [])
        .map_err(|e| format!("回填全文索引失败: {e}"))?;
    Ok(())
}

/// 事务内替换一个文档的 chunks（含向量与全文行）。
pub fn replace_chunks(
    tx: &rusqlite::Transaction,
    document_id: &str,
    chunks: &[TextChunk],
    vectors: &[Vec<f32>],
) -> Result<(), String> {
    if chunks.len() != vectors.len() {
        return Err(format!(
            "chunks 与向量数量不一致: {} vs {}",
            chunks.len(),
            vectors.len()
        ));
    }
    tx.execute(
        "DELETE FROM chunks_fts WHERE rowid IN (SELECT id FROM chunks WHERE document_id = ?1)",
        [document_id],
    )
    .map_err(|e| format!("清理全文索引失败: {e}"))?;
    // 必须先清向量行：chunks 删除后 rowid 会被复用，残留向量会撞 vec0 主键
    tx.execute(
        "DELETE FROM chunks_vec WHERE chunk_id IN (SELECT id FROM chunks WHERE document_id = ?1)",
        [document_id],
    )
    .map_err(|e| format!("清理旧向量失败: {e}"))?;
    tx.execute("DELETE FROM chunks WHERE document_id = ?1", [document_id])
        .map_err(|e| format!("清理旧 chunks 失败: {e}"))?;

    for (chunk, vector) in chunks.iter().zip(vectors.iter()) {
        let serialized = serde_json::to_string(vector).map_err(|e| format!("序列化向量失败: {e}"))?;
        tx.execute(
            "INSERT INTO chunks (document_id, label, text, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![document_id, chunk.label, chunk.text, now_rfc3339()],
        )
        .map_err(|e| format!("写入 chunk 失败: {e}"))?;
        let chunk_id = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO chunks_vec (chunk_id, embedding) VALUES (?1, ?2)",
            params![chunk_id, serialized],
        )
        .map_err(|e| format!("写入向量失败: {e}"))?;
        tx.execute(
            "INSERT INTO chunks_fts (rowid, text) VALUES (?1, ?2)",
            params![chunk_id, chunk.text],
        )
        .map_err(|e| format!("写入全文索引失败: {e}"))?;
    }
    Ok(())
}

/// 删除一个文档的全部切片（fts → vec → chunks，与 replace_chunks 顺序一致）。
/// 供 force 重编译在写入新编译稿前清理旧产物；索引 schema 未建或无切片时为幂等空操作。
pub fn delete_document_chunks(tx: &rusqlite::Transaction, document_id: &str) -> Result<(), String> {
    let table_exists = |name: &str| -> Result<bool, String> {
        tx.query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [name],
            |_| Ok(()),
        )
        .optional()
        .map(|o| o.is_some())
        .map_err(|e| format!("检查表失败: {e}"))
    };
    if table_exists("chunks_fts")? {
        tx.execute(
            "DELETE FROM chunks_fts WHERE rowid IN (SELECT id FROM chunks WHERE document_id = ?1)",
            [document_id],
        )
        .map_err(|e| format!("清理全文索引失败: {e}"))?;
    }
    if table_exists("chunks_vec")? {
        tx.execute(
            "DELETE FROM chunks_vec WHERE chunk_id IN (SELECT id FROM chunks WHERE document_id = ?1)",
            [document_id],
        )
        .map_err(|e| format!("清理旧向量失败: {e}"))?;
    }
    if table_exists("chunks")? {
        tx.execute("DELETE FROM chunks WHERE document_id = ?1", [document_id])
            .map_err(|e| format!("清理旧 chunks 失败: {e}"))?;
    }
    Ok(())
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

    fn setup() -> TempDir {
        let tmp = TempDir::new().unwrap();
        ensure_db(tmp.path()).unwrap();
        tmp
    }

    fn chunk(label: &str, text: &str) -> TextChunk {
        TextChunk {
            label: label.to_string(),
            text: text.to_string(),
        }
    }

    fn counts(tmp: &TempDir) -> (i64, i64, i64) {
        let conn = Connection::open(crate::db::db_path(tmp.path())).unwrap();
        let c = |sql: &str| -> i64 { conn.query_row(sql, [], |r| r.get(0)).unwrap() };
        (
            c("SELECT COUNT(*) FROM chunks"),
            c("SELECT COUNT(*) FROM chunks_vec"),
            c("SELECT COUNT(*) FROM chunks_fts"),
        )
    }

    fn insert_doc(tmp: &TempDir, id: &str, status: &str) {
        let conn = Connection::open(crate::db::db_path(tmp.path())).unwrap();
        conn.execute(
            "INSERT INTO documents (id, file_name, mime_type, type, sha1, size, content, status, created_at, updated_at)
             VALUES (?1, ?1, 'text/markdown', 'plaintext', ?1, 1, 'x', ?2, 't', 't')",
            params![id, status],
        )
        .unwrap();
    }

    #[test]
    fn test_create_vector_schema_and_fts() {
        let tmp = setup();
        let conn = open(tmp.path()).unwrap();

        create_vector_schema(&conn, 4).unwrap();

        let dims: String = conn
            .query_row(
                "SELECT value FROM index_meta WHERE key = 'embedding_dimension'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(dims, "4");
        // fts 可写入并 trigram 查询（trigram 查询串需 ≥3 字符）
        conn.execute("INSERT INTO chunks_fts(rowid, text) VALUES (1, '中文短串匹配测试')", [])
            .unwrap();
        let hit: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM chunks_fts WHERE chunks_fts MATCH '短串匹'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(hit, 1);
    }

    #[test]
    fn test_create_vector_schema_idempotent() {
        let tmp = setup();
        let conn = open(tmp.path()).unwrap();
        create_vector_schema(&conn, 4).unwrap();
        create_vector_schema(&conn, 4).unwrap();
        assert_eq!(counts(&tmp), (0, 0, 0));
    }

    #[test]
    fn test_dimension_change_resets_chunks_and_status() {
        let tmp = setup();
        insert_doc(&tmp, "a", "indexed");
        {
            let conn = open(tmp.path()).unwrap();
            create_vector_schema(&conn, 4).unwrap();
            conn.execute(
                "INSERT INTO chunks (document_id, label, text, created_at) VALUES ('a', 'l', 't', 't')",
                [],
            )
            .unwrap();
        }
        {
            let conn = open(tmp.path()).unwrap();
            create_vector_schema(&conn, 8).unwrap();
        }
        let conn = Connection::open(crate::db::db_path(tmp.path())).unwrap();
        let (chunks, status): (i64, String) = conn
            .query_row(
                "SELECT (SELECT COUNT(*) FROM chunks), (SELECT status FROM documents WHERE id = 'a')",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(chunks, 0);
        assert_eq!(status, "compiled");
        let dims: String = conn
            .query_row(
                "SELECT value FROM index_meta WHERE key = 'embedding_dimension'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(dims, "8");
    }

    #[test]
    fn test_replace_chunks_writes_three_tables() {
        let tmp = setup();
        insert_doc(&tmp, "a", "compiled");
        let mut conn = open(tmp.path()).unwrap();
        create_vector_schema(&conn, 2).unwrap();
        let tx = conn.transaction().unwrap();

        replace_chunks(
            &tx,
            "a",
            &[chunk("甲", "第一块"), chunk("乙", "第二块")],
            &[vec![1.0, 0.0], vec![0.0, 1.0]],
        )
        .unwrap();
        tx.commit().unwrap();

        assert_eq!(counts(&tmp), (2, 2, 2));
        let conn = Connection::open(crate::db::db_path(tmp.path())).unwrap();
        let label: String = conn
            .query_row("SELECT label FROM chunks WHERE id = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(label, "甲");
        let fts_hit: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM chunks_fts WHERE chunks_fts MATCH '第二块'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(fts_hit, 1);
    }

    #[test]
    fn test_replace_chunks_replaces_previous() {
        let tmp = setup();
        insert_doc(&tmp, "a", "compiled");
        let mut conn = open(tmp.path()).unwrap();
        create_vector_schema(&conn, 2).unwrap();
        {
            let tx = conn.transaction().unwrap();
            replace_chunks(&tx, "a", &[chunk("旧", "旧内容")], &[vec![1.0, 0.0]]).unwrap();
            tx.commit().unwrap();
        }
        {
            let tx = conn.transaction().unwrap();
            replace_chunks(
                &tx,
                "a",
                &[chunk("新一", "新内容甲"), chunk("新二", "新内容乙")],
                &[vec![1.0, 0.0], vec![0.0, 1.0]],
            )
            .unwrap();
            tx.commit().unwrap();
        }
        assert_eq!(counts(&tmp), (2, 2, 2));
    }

    #[test]
    fn test_replace_chunks_length_mismatch_is_error() {
        let tmp = setup();
        insert_doc(&tmp, "a", "compiled");
        let mut conn = open(tmp.path()).unwrap();
        create_vector_schema(&conn, 2).unwrap();
        let tx = conn.transaction().unwrap();
        let err = replace_chunks(&tx, "a", &[chunk("x", "y")], &[]).unwrap_err();
        assert!(err.contains("数量不一致"));
    }

    #[test]
    fn test_fts_backfills_existing_chunks() {
        let tmp = setup();
        insert_doc(&tmp, "a", "compiled");
        {
            let conn = open(tmp.path()).unwrap();
            ensure_chunks_table(&conn).unwrap();
            conn.execute(
                "INSERT INTO chunks (document_id, label, text, created_at) VALUES ('a', 'l', '回填文本', 't')",
                [],
            )
            .unwrap();
        }
        let conn = open(tmp.path()).unwrap();
        ensure_fts_schema(&conn).unwrap();
        let hit: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM chunks_fts WHERE chunks_fts MATCH '回填文'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(hit, 1);
    }
}
