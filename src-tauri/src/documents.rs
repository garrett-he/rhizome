use std::{
    fs,
    path::{Path, PathBuf},
};

use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;
use sha1::{Digest, Sha1};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

pub const TABLE_SQL: &str = "CREATE TABLE IF NOT EXISTS documents (
    id TEXT PRIMARY KEY,
    file_name TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    type TEXT NOT NULL,
    sha1 TEXT NOT NULL UNIQUE,
    size INTEGER NOT NULL,
    content TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'raw',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    content_compiled TEXT,
    metadata_compiled TEXT,
    compiled_at TEXT,
    llm_model TEXT,
    indexed_at TEXT,
    chunk_count INTEGER,
    embedding_model TEXT,
    last_error TEXT
);";

/// 旧库（最小列集）幂等补列：(列名, 列声明)
const MIGRATION_COLUMNS: &[(&str, &str)] = &[
    ("content_compiled", "TEXT"),
    ("metadata_compiled", "TEXT"),
    ("compiled_at", "TEXT"),
    ("llm_model", "TEXT"),
    ("indexed_at", "TEXT"),
    ("chunk_count", "INTEGER"),
    ("embedding_model", "TEXT"),
    ("last_error", "TEXT"),
];

#[derive(Serialize, Debug, PartialEq, Clone)]
pub struct DocumentSummary {
    pub id: String,
    pub file_name: String,
    pub mime_type: String,
    #[serde(rename = "type")]
    pub doc_type: String,
    pub status: String,
    pub size: u64,
    pub created_at: String,
    pub last_error: Option<String>,
}

/// 文档详情（详情页全量数据）。
#[derive(Serialize, Debug, PartialEq, Clone)]
pub struct DocumentDetail {
    pub id: String,
    pub file_name: String,
    pub mime_type: String,
    #[serde(rename = "type")]
    pub doc_type: String,
    pub sha1: String,
    pub size: u64,
    pub status: String,
    pub content: String,
    pub content_compiled: Option<String>,
    pub metadata_compiled: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub compiled_at: Option<String>,
    pub llm_model: Option<String>,
    pub indexed_at: Option<String>,
    pub chunk_count: Option<i64>,
    pub embedding_model: Option<String>,
    pub last_error: Option<String>,
    /// blob 原始文件绝对路径（打开原始文件用）
    pub blob_path: String,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct AddFailure {
    pub path: String,
    pub error: String,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct AddSummary {
    pub added: u32,
    pub skipped: u32,
    pub failures: Vec<AddFailure>,
}

#[derive(Debug, PartialEq)]
enum AddOutcome {
    Added { id: String },
    Skipped { id: String },
}

pub fn ensure_table(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(TABLE_SQL)
        .map_err(|e| format!("初始化 documents 表失败: {e}"))?;
    migrate_columns(conn)
}

/// 对旧库幂等补列：按 PRAGMA table_info 检查缺失列再 ALTER TABLE ADD COLUMN
fn migrate_columns(conn: &Connection) -> Result<(), String> {
    let existing = column_names(conn)?;
    for (name, decl) in MIGRATION_COLUMNS {
        if existing.iter().any(|c| c == name) {
            continue;
        }
        conn.execute(&format!("ALTER TABLE documents ADD COLUMN {name} {decl}"), [])
            .map_err(|e| format!("迁移 documents 表失败: {e}"))?;
    }
    Ok(())
}

fn column_names(conn: &Connection) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare("PRAGMA table_info(documents)")
        .map_err(|e| format!("检查 documents 表结构失败: {e}"))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| format!("检查 documents 表结构失败: {e}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("检查 documents 表结构失败: {e}"))
}

fn now_rfc3339() -> String {
    OffsetDateTime::now_utc().format(&Rfc3339).unwrap_or_default()
}

fn sha1_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

/// 仅按扩展名判型（本期全部为纯文本，魔数探测留待引入二进制类型时补充）
fn detect_mime(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "md" => Some("text/markdown"),
        "txt" => Some("text/plain"),
        "srt" => Some("application/x-subrip"),
        _ => None,
    }
}

fn blob_path(vault: &Path, sha1: &str) -> PathBuf {
    vault.join("blobs").join(&sha1[0..2]).join(&sha1[2..4]).join(sha1)
}

/// 添加单个文档（对齐 deprecated add 流程）：sha1 去重跳过、UTF-8 lossy 读全文、
/// 复制原文到 blobs 分片目录、插入 status='raw' 行；DB 插入失败回滚删除 blob。
fn add(vault: &Path, path: &str) -> Result<AddOutcome, String> {
    let trimmed = path.trim();
    let source = Path::new(trimmed);
    let Some(mime_type) = detect_mime(source) else {
        return Err(format!("不支持的文件类型: {trimmed}"));
    };

    let bytes = fs::read(source).map_err(|e| format!("读取文件失败: {trimmed}: {e}"))?;
    let digest = sha1_hex(&bytes);

    let conn = Connection::open(crate::db::db_path(vault)).map_err(|e| format!("打开数据库失败: {e}"))?;
    let existing: Option<String> = conn
        .query_row("SELECT id FROM documents WHERE sha1 = ?1", [&digest], |row| row.get(0))
        .optional()
        .map_err(|e| format!("查询失败: {e}"))?;
    if let Some(id) = existing {
        return Ok(AddOutcome::Skipped { id });
    }

    let content = String::from_utf8_lossy(&bytes).into_owned();
    let size = bytes.len() as u64;
    let file_name = source
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| trimmed.to_string());

    let blob = blob_path(vault, &digest);
    if let Some(parent) = blob.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建 blob 目录失败: {e}"))?;
    }
    fs::copy(source, &blob).map_err(|e| format!("复制原文失败: {trimmed}: {e}"))?;

    let id = Uuid::new_v4().to_string().replace('-', "");
    let now = now_rfc3339();
    let result = conn.execute(
        "INSERT INTO documents (id, file_name, mime_type, type, sha1, size, content, status, created_at, updated_at)
         VALUES (?1, ?2, ?3, 'plaintext', ?4, ?5, ?6, 'raw', ?7, ?7)",
        rusqlite::params![id, file_name, mime_type, digest, size, content, now],
    );
    if let Err(e) = result {
        let _ = fs::remove_file(&blob);
        return Err(format!("写入数据库失败: {e}"));
    }
    Ok(AddOutcome::Added { id })
}

/// 批量添加：逐个执行，单文件失败不中断批次
pub fn add_batch(vault: &Path, paths: &[String]) -> AddSummary {
    let mut summary = AddSummary {
        added: 0,
        skipped: 0,
        failures: Vec::new(),
    };
    for path in paths {
        match add(vault, path) {
            Ok(AddOutcome::Added { .. }) => summary.added += 1,
            Ok(AddOutcome::Skipped { .. }) => summary.skipped += 1,
            Err(error) => summary.failures.push(AddFailure {
                path: path.clone(),
                error,
            }),
        }
    }
    summary
}

pub fn list(vault: &Path) -> Result<Vec<DocumentSummary>, String> {
    let conn = Connection::open(crate::db::db_path(vault)).map_err(|e| format!("打开数据库失败: {e}"))?;
    ensure_table(&conn)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, file_name, mime_type, type, status, size, created_at, last_error
             FROM documents ORDER BY created_at DESC",
        )
        .map_err(|e| format!("查询失败: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(DocumentSummary {
                id: row.get(0)?,
                file_name: row.get(1)?,
                mime_type: row.get(2)?,
                doc_type: row.get(3)?,
                status: row.get(4)?,
                size: row.get::<_, i64>(5)?.max(0) as u64,
                created_at: row.get(6)?,
                last_error: row.get(7)?,
            })
        })
        .map_err(|e| format!("查询失败: {e}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("读取结果失败: {e}"))
}

/// 读取单个文档详情；不存在返回 Err。
pub fn get_detail(vault: &Path, id: &str) -> Result<DocumentDetail, String> {
    let conn = Connection::open(crate::db::db_path(vault)).map_err(|e| format!("打开数据库失败: {e}"))?;
    ensure_table(&conn)?;
    let detail = conn
        .query_row(
            "SELECT id, file_name, mime_type, type, sha1, size, status, content, content_compiled,
                    metadata_compiled, created_at, updated_at, compiled_at, llm_model, indexed_at,
                    chunk_count, embedding_model, last_error
             FROM documents WHERE id = ?1",
            [id],
            |row| {
                Ok(DocumentDetail {
                    id: row.get(0)?,
                    file_name: row.get(1)?,
                    mime_type: row.get(2)?,
                    doc_type: row.get(3)?,
                    sha1: row.get(4)?,
                    size: row.get::<_, i64>(5)?.max(0) as u64,
                    status: row.get(6)?,
                    content: row.get(7)?,
                    content_compiled: row.get(8)?,
                    metadata_compiled: row.get(9)?,
                    created_at: row.get(10)?,
                    updated_at: row.get(11)?,
                    compiled_at: row.get(12)?,
                    llm_model: row.get(13)?,
                    indexed_at: row.get(14)?,
                    chunk_count: row.get(15)?,
                    embedding_model: row.get(16)?,
                    last_error: row.get(17)?,
                    blob_path: String::new(),
                })
            },
        )
        .optional()
        .map_err(|e| format!("查询失败: {e}"))?
        .ok_or_else(|| format!("文档不存在: {id}"))?;
    Ok(DocumentDetail {
        blob_path: blob_path(vault, &detail.sha1).to_string_lossy().into_owned(),
        ..detail
    })
}

/// (total, indexed, pending)；documents 表缺失时全部为 0（防御旧库未迁移）
pub fn counts(conn: &Connection) -> (u64, u64, u64) {
    let table_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'documents'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map(|n| n > 0)
        .unwrap_or(false);
    if !table_exists {
        return (0, 0, 0);
    }
    let one = |sql: &str| -> u64 { conn.query_row(sql, [], |row| row.get::<_, i64>(0)).unwrap_or(0).max(0) as u64 };
    (
        one("SELECT COUNT(*) FROM documents"),
        one("SELECT COUNT(*) FROM documents WHERE status = 'indexed'"),
        one("SELECT COUNT(*) FROM documents WHERE status = 'raw'"),
    )
}

/// last_error 非空的文档数（处理失败）；表缺失时为 0
pub fn failed_count(conn: &Connection) -> u64 {
    let table_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'documents'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map(|n| n > 0)
        .unwrap_or(false);
    if !table_exists {
        return 0;
    }
    conn.query_row(
        "SELECT COUNT(*) FROM documents WHERE last_error IS NOT NULL",
        [],
        |row| row.get::<_, i64>(0),
    )
    .unwrap_or(0)
    .max(0) as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{db_path, ensure_db};
    use tempfile::TempDir;

    fn setup() -> TempDir {
        let tmp = TempDir::new().unwrap();
        ensure_db(tmp.path()).unwrap();
        tmp
    }

    fn write_file(dir: &Path, name: &str, content: &[u8]) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn test_add_plaintext_document() {
        let tmp = setup();
        let file = write_file(tmp.path(), "notes.md", b"# hello");

        match add(tmp.path(), file.to_str().unwrap()).unwrap() {
            AddOutcome::Added { id } => assert_eq!(id.len(), 32),
            other => panic!("期望 Added，实际 {other:?}"),
        }

        let conn = Connection::open(db_path(tmp.path())).unwrap();
        let (file_name, mime_type, doc_type, status, size, content): (String, String, String, String, i64, String) =
            conn.query_row(
                "SELECT file_name, mime_type, type, status, size, content FROM documents",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(file_name, "notes.md");
        assert_eq!(mime_type, "text/markdown");
        assert_eq!(doc_type, "plaintext");
        assert_eq!(status, "raw");
        assert_eq!(size, 7);
        assert_eq!(content, "# hello");
    }

    #[test]
    fn test_add_copies_blob() {
        let tmp = setup();
        let file = write_file(tmp.path(), "a.txt", b"content");

        add(tmp.path(), file.to_str().unwrap()).unwrap();

        let conn = Connection::open(db_path(tmp.path())).unwrap();
        let sha1: String = conn
            .query_row("SELECT sha1 FROM documents", [], |row| row.get(0))
            .unwrap();
        let blob = blob_path(tmp.path(), &sha1);
        assert!(blob.is_file());
        assert_eq!(fs::read(&blob).unwrap(), b"content");
    }

    #[test]
    fn test_add_duplicate_is_skipped() {
        let tmp = setup();
        let file = write_file(tmp.path(), "a.txt", b"same");
        let first = add(tmp.path(), file.to_str().unwrap()).unwrap();
        let second = add(tmp.path(), file.to_str().unwrap()).unwrap();

        match (first, second) {
            (AddOutcome::Added { id: a }, AddOutcome::Skipped { id: b }) => assert_eq!(a, b),
            other => panic!("期望 Added + Skipped，实际 {other:?}"),
        }
        let conn = Connection::open(db_path(tmp.path())).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM documents", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_add_unsupported_extension_fails() {
        let tmp = setup();
        let file = write_file(tmp.path(), "image.png", b"\x89PNG");
        let err = add(tmp.path(), file.to_str().unwrap()).unwrap_err();

        assert!(err.contains("不支持的文件类型"));
        let conn = Connection::open(db_path(tmp.path())).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM documents", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_add_missing_file_fails() {
        let tmp = setup();
        let err = add(tmp.path(), tmp.path().join("nope.md").to_str().unwrap()).unwrap_err();
        assert!(err.contains("读取文件失败"));
    }

    #[test]
    fn test_mime_detection_by_extension() {
        assert_eq!(detect_mime(Path::new("a.md")), Some("text/markdown"));
        assert_eq!(detect_mime(Path::new("a.MD")), Some("text/markdown"));
        assert_eq!(detect_mime(Path::new("a.txt")), Some("text/plain"));
        assert_eq!(detect_mime(Path::new("a.srt")), Some("application/x-subrip"));
        assert_eq!(detect_mime(Path::new("a.html")), None);
        assert_eq!(detect_mime(Path::new("noext")), None);
    }

    #[test]
    fn test_add_batch_aggregates_results() {
        let tmp = setup();
        let a = write_file(tmp.path(), "a.md", b"a");
        let b = write_file(tmp.path(), "b.txt", b"b");
        let bad = write_file(tmp.path(), "c.png", b"c");
        let paths = [
            a.to_str().unwrap().to_string(),
            b.to_str().unwrap().to_string(),
            bad.to_str().unwrap().to_string(),
            "missing.md".to_string(),
        ];

        let summary = add_batch(tmp.path(), &paths);
        assert_eq!(summary.added, 2);
        assert_eq!(summary.skipped, 0);
        assert_eq!(summary.failures.len(), 2);
        assert!(summary.failures[0].error.contains("不支持的文件类型"));
        assert!(summary.failures[1].error.contains("读取文件失败"));
    }

    #[test]
    fn test_list_orders_newest_first() {
        let tmp = setup();
        let a = write_file(tmp.path(), "a.md", b"first");
        let b = write_file(tmp.path(), "b.txt", b"second");
        add_batch(
            tmp.path(),
            &[a.to_str().unwrap().to_string(), b.to_str().unwrap().to_string()],
        );

        let docs = list(tmp.path()).unwrap();
        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].file_name, "b.txt");
        assert_eq!(docs[1].file_name, "a.md");
    }

    #[test]
    fn test_get_detail_returns_full_record() {
        let tmp = setup();
        let file = write_file(tmp.path(), "notes.md", b"# hello");
        let id = match add(tmp.path(), file.to_str().unwrap()).unwrap() {
            AddOutcome::Added { id } => id,
            other => panic!("期望 Added，实际 {other:?}"),
        };
        let conn = Connection::open(db_path(tmp.path())).unwrap();
        conn.execute(
            "UPDATE documents SET status = 'compiled', content_compiled = '# 编译稿', metadata_compiled = '{\"title\":\"编译标题\"}' WHERE id = ?1",
            [&id],
        )
        .unwrap();

        let detail = get_detail(tmp.path(), &id).unwrap();
        assert_eq!(detail.id, id);
        assert_eq!(detail.file_name, "notes.md");
        assert_eq!(detail.doc_type, "plaintext");
        assert_eq!(detail.status, "compiled");
        assert_eq!(detail.content, "# hello");
        assert_eq!(detail.content_compiled.as_deref(), Some("# 编译稿"));
        assert_eq!(detail.metadata_compiled.as_deref(), Some("{\"title\":\"编译标题\"}"));
        assert_eq!(detail.sha1.len(), 40);
        // Windows 路径分隔符归一后校验分片目录结构
        let normalized = detail.blob_path.replace('\\', "/");
        assert!(normalized.contains(&format!("blobs/{}/{}", &detail.sha1[0..2], &detail.sha1[2..4])));
        assert!(detail.blob_path.ends_with(&detail.sha1));
        assert_eq!(detail.chunk_count, None);
        assert_eq!(detail.last_error, None);
    }

    #[test]
    fn test_get_detail_missing_is_error() {
        let tmp = setup();
        let error = get_detail(tmp.path(), "nope").unwrap_err();
        assert!(error.contains("文档不存在"));
    }

    #[test]
    fn test_counts_reflect_documents() {
        let tmp = setup();
        let a = write_file(tmp.path(), "a.md", b"one");
        let b = write_file(tmp.path(), "b.txt", b"two");
        add_batch(
            tmp.path(),
            &[a.to_str().unwrap().to_string(), b.to_str().unwrap().to_string()],
        );

        let conn = Connection::open(db_path(tmp.path())).unwrap();
        let (total, indexed, pending) = counts(&conn);
        assert_eq!((total, indexed, pending), (2, 0, 2));
    }

    #[test]
    fn test_counts_without_documents_table_is_zero() {
        let tmp = TempDir::new().unwrap();
        ensure_db(tmp.path()).unwrap();
        let conn = Connection::open(db_path(tmp.path())).unwrap();
        conn.execute("DROP TABLE documents", []).unwrap();

        assert_eq!(counts(&conn), (0, 0, 0));
    }

    #[test]
    fn test_ensure_table_migrates_minimal_schema() {
        let tmp = TempDir::new().unwrap();
        let conn = Connection::open(db_path(tmp.path())).unwrap();
        conn.execute(
            "CREATE TABLE documents (
                id TEXT PRIMARY KEY,
                file_name TEXT NOT NULL,
                mime_type TEXT NOT NULL,
                type TEXT NOT NULL,
                sha1 TEXT NOT NULL UNIQUE,
                size INTEGER NOT NULL,
                content TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'raw',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
            [],
        )
        .unwrap();

        ensure_table(&conn).unwrap();

        let names = column_names(&conn).unwrap();
        for (name, _) in MIGRATION_COLUMNS {
            assert!(names.iter().any(|c| c == name), "缺少列 {name}");
        }
        // 再跑一遍仍是幂等的
        ensure_table(&conn).unwrap();
    }

    #[test]
    fn test_list_returns_last_error() {
        let tmp = setup();
        let file = write_file(tmp.path(), "a.md", b"a");
        add(tmp.path(), file.to_str().unwrap()).unwrap();
        let conn = Connection::open(db_path(tmp.path())).unwrap();
        conn.execute("UPDATE documents SET last_error = 'compile: boom'", [])
            .unwrap();

        let docs = list(tmp.path()).unwrap();
        assert_eq!(docs[0].last_error.as_deref(), Some("compile: boom"));
    }
}
