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

/// 文档分类关联索引（幂等建，category_id 补列之后执行）
pub const CATEGORY_INDEX_SQL: &str = "CREATE INDEX IF NOT EXISTS idx_documents_category_id ON documents(category_id);";

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
    ("category_id", "TEXT"),
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
    /// 所属分类（LEFT JOIN categories 取名）
    pub category_id: Option<String>,
    pub category_name: Option<String>,
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
    /// 所属分类（LEFT JOIN categories 取名）
    pub category_id: Option<String>,
    pub category_name: Option<String>,
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
    migrate_columns(conn)?;
    conn.execute_batch(CATEGORY_INDEX_SQL)
        .map_err(|e| format!("创建 documents 分类索引失败: {e}"))
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

/// 按扩展名判型（魔数探测留待后续按需补充）。md/txt/srt 为纯文本直读，
/// 其余类型经 extractor 抽取为 Markdown/OCR/SRT 文本。
fn detect_mime(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        // 纯文本
        "md" => Some("text/markdown"),
        "txt" => Some("text/plain"),
        "srt" => Some("application/x-subrip"),
        // 文档
        "docx" => Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document"),
        "pptx" => Some("application/vnd.openxmlformats-officedocument.presentationml.presentation"),
        "xlsx" | "xlsm" => Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
        "pdf" => Some("application/pdf"),
        "html" | "htm" => Some("text/html"),
        // 图片
        "bmp" => Some("image/bmp"),
        "jpeg" | "jpg" => Some("image/jpeg"),
        "png" => Some("image/png"),
        "tiff" => Some("image/tiff"),
        "webp" => Some("image/webp"),
        // 音频
        "aac" => Some("audio/aac"),
        "aiff" => Some("audio/aiff"),
        "amr" => Some("audio/amr"),
        "flac" => Some("audio/flac"),
        "m4a" => Some("audio/mp4"),
        "mp3" => Some("audio/mpeg"),
        "ogg" => Some("audio/ogg"),
        "opus" => Some("audio/opus"),
        "wav" => Some("audio/wav"),
        "wma" => Some("audio/x-ms-wma"),
        // 视频
        "3gp" => Some("video/3gpp"),
        "avi" => Some("video/x-msvideo"),
        "flv" => Some("video/x-flv"),
        "m4v" => Some("video/x-m4v"),
        "mkv" => Some("video/x-matroska"),
        "mov" => Some("video/quicktime"),
        "mp4" => Some("video/mp4"),
        "mpg" | "mpeg" => Some("video/mpeg"),
        "ts" => Some("video/mp2t"),
        "webm" => Some("video/webm"),
        "wmv" => Some("video/x-ms-wmv"),
        _ => None,
    }
}

fn is_plain_text(mime_type: &str) -> bool {
    mime_type.starts_with("text/") || mime_type == "application/x-subrip"
}

fn blob_path(vault: &Path, sha1: &str) -> PathBuf {
    vault.join("blobs").join(&sha1[0..2]).join(&sha1[2..4]).join(sha1)
}

/// 文本抽取入口（依赖注入）：lib.rs 注入 extract::CliExtractor，测试注入 fake。
pub trait ExtractsText {
    fn extract(&self, vault: &Path, path: &Path, timeout: std::time::Duration) -> Result<String, String>;
}

/// 添加单个文档（对齐 deprecated add 流程）：sha1 去重跳过、纯文本直读 /
/// 其余类型经 extractor 抽取为文本、复制原文到 blobs 分片目录、插入
/// status='raw' 行；DB 插入失败回滚删除 blob。
fn add(
    vault: &Path,
    path: &str,
    category_id: Option<&str>,
    extractor: &dyn ExtractsText,
) -> Result<AddOutcome, String> {
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

    let content = if is_plain_text(mime_type) {
        String::from_utf8_lossy(&bytes).into_owned()
    } else {
        let timeout = if mime_type.starts_with("audio/") || mime_type.starts_with("video/") {
            crate::extract::TIMEOUT_MEDIA
        } else {
            crate::extract::TIMEOUT_TEXT
        };
        extractor.extract(vault, source, timeout)?
    };
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
        "INSERT INTO documents (id, file_name, mime_type, type, sha1, size, content, status, created_at, updated_at, category_id)
         VALUES (?1, ?2, ?3, 'plaintext', ?4, ?5, ?6, 'raw', ?7, ?7, ?8)",
        rusqlite::params![id, file_name, mime_type, digest, size, content, now, category_id],
    );
    if let Err(e) = result {
        let _ = fs::remove_file(&blob);
        return Err(format!("写入数据库失败: {e}"));
    }
    Ok(AddOutcome::Added { id })
}

/// 批量添加：逐个执行，单文件失败不中断批次；可选统一归入指定分类
pub fn add_batch(vault: &Path, paths: &[String], category_id: Option<&str>, extractor: &dyn ExtractsText) -> AddSummary {
    let mut summary = AddSummary {
        added: 0,
        skipped: 0,
        failures: Vec::new(),
    };
    for path in paths {
        match add(vault, path, category_id, extractor) {
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
    crate::categories::ensure_table(&conn)?;
    let mut stmt = conn
        .prepare(
            "SELECT d.id, d.file_name, d.mime_type, d.type, d.status, d.size, d.created_at, d.last_error,
                    d.category_id, c.name
             FROM documents d LEFT JOIN categories c ON c.id = d.category_id
             ORDER BY d.created_at DESC",
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
                category_id: row.get(8)?,
                category_name: row.get(9)?,
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
    crate::categories::ensure_table(&conn)?;
    let detail = conn
        .query_row(
            "SELECT d.id, d.file_name, d.mime_type, d.type, d.sha1, d.size, d.status, d.content, d.content_compiled,
                    d.metadata_compiled, d.created_at, d.updated_at, d.compiled_at, d.llm_model, d.indexed_at,
                    d.chunk_count, d.embedding_model, d.last_error, d.category_id, c.name
             FROM documents d LEFT JOIN categories c ON c.id = d.category_id
             WHERE d.id = ?1",
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
                    category_id: row.get(18)?,
                    category_name: row.get(19)?,
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

/// 设置/更换/清除文档分类（category_id 为 None 时清除）；文档或分类不存在返回 Err。
pub fn set_category(vault: &Path, document_id: &str, category_id: Option<&str>) -> Result<(), String> {
    let conn = Connection::open(crate::db::db_path(vault)).map_err(|e| format!("打开数据库失败: {e}"))?;
    ensure_table(&conn)?;
    crate::categories::ensure_table(&conn)?;
    if let Some(cid) = category_id {
        let exists: Option<i64> = conn
            .query_row("SELECT 1 FROM categories WHERE id = ?1", [cid], |row| row.get(0))
            .optional()
            .map_err(|e| format!("查询失败: {e}"))?;
        if exists.is_none() {
            return Err(format!("分类不存在: {cid}"));
        }
    }
    let updated = conn
        .execute(
            "UPDATE documents SET category_id = ?1 WHERE id = ?2",
            rusqlite::params![category_id, document_id],
        )
        .map_err(|e| format!("写入数据库失败: {e}"))?;
    if updated == 0 {
        return Err(format!("文档不存在: {document_id}"));
    }
    Ok(())
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

    /// 纯文本测试用：任何抽取调用都是 bug。
    struct NoExtractor;

    impl ExtractsText for NoExtractor {
        fn extract(
            &self,
            _vault: &Path,
            path: &Path,
            _timeout: std::time::Duration,
        ) -> Result<String, String> {
            panic!("unexpected extraction call: {path:?}");
        }
    }

    /// 非纯文本测试用：返回固定文本并记录收到的超时。
    struct StubExtractor {
        output: String,
    }

    impl StubExtractor {
        fn new(output: &str) -> Self {
            Self {
                output: output.to_string(),
            }
        }
    }

    impl ExtractsText for StubExtractor {
        fn extract(
            &self,
            _vault: &Path,
            _path: &Path,
            _timeout: std::time::Duration,
        ) -> Result<String, String> {
            Ok(self.output.clone())
        }
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

        match add(tmp.path(), file.to_str().unwrap(), None, &NoExtractor).unwrap() {
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

        add(tmp.path(), file.to_str().unwrap(), None, &NoExtractor).unwrap();

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
        let first = add(tmp.path(), file.to_str().unwrap(), None, &NoExtractor).unwrap();
        let second = add(tmp.path(), file.to_str().unwrap(), None, &NoExtractor).unwrap();

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
        let file = write_file(tmp.path(), "archive.xyz", b"???");
        let err = add(tmp.path(), file.to_str().unwrap(), None, &NoExtractor).unwrap_err();

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
        let err = add(tmp.path(), tmp.path().join("nope.md").to_str().unwrap(), None, &NoExtractor).unwrap_err();
        assert!(err.contains("读取文件失败"));
    }

    #[test]
    fn test_mime_detection_by_extension() {
        assert_eq!(detect_mime(Path::new("a.md")), Some("text/markdown"));
        assert_eq!(detect_mime(Path::new("a.MD")), Some("text/markdown"));
        assert_eq!(detect_mime(Path::new("a.txt")), Some("text/plain"));
        assert_eq!(detect_mime(Path::new("a.srt")), Some("application/x-subrip"));
        assert_eq!(
            detect_mime(Path::new("a.docx")),
            Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document")
        );
        assert_eq!(
            detect_mime(Path::new("a.pptx")),
            Some("application/vnd.openxmlformats-officedocument.presentationml.presentation")
        );
        assert_eq!(
            detect_mime(Path::new("a.xlsx")),
            Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet")
        );
        assert_eq!(detect_mime(Path::new("a.pdf")), Some("application/pdf"));
        assert_eq!(detect_mime(Path::new("a.html")), Some("text/html"));
        assert_eq!(detect_mime(Path::new("a.png")), Some("image/png"));
        assert_eq!(detect_mime(Path::new("a.mp3")), Some("audio/mpeg"));
        assert_eq!(detect_mime(Path::new("a.mp4")), Some("video/mp4"));
        assert_eq!(detect_mime(Path::new("noext")), None);
    }

    #[test]
    fn test_add_batch_aggregates_results() {
        let tmp = setup();
        let a = write_file(tmp.path(), "a.md", b"a");
        let b = write_file(tmp.path(), "b.txt", b"b");
        let bad = write_file(tmp.path(), "c.xyz", b"c");
        let paths = [
            a.to_str().unwrap().to_string(),
            b.to_str().unwrap().to_string(),
            bad.to_str().unwrap().to_string(),
            "missing.md".to_string(),
        ];

        let summary = add_batch(tmp.path(), &paths, None, &NoExtractor);
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
            None,
            &NoExtractor,
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
        let id = match add(tmp.path(), file.to_str().unwrap(), None, &NoExtractor).unwrap() {
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
            None,
            &NoExtractor,
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
        add(tmp.path(), file.to_str().unwrap(), None, &NoExtractor).unwrap();
        let conn = Connection::open(db_path(tmp.path())).unwrap();
        conn.execute("UPDATE documents SET last_error = 'compile: boom'", [])
            .unwrap();

        let docs = list(tmp.path()).unwrap();
        assert_eq!(docs[0].last_error.as_deref(), Some("compile: boom"));
    }

    #[test]
    fn test_add_with_category_and_list_shows_name() {
        let tmp = setup();
        let category = crate::categories::create_category(tmp.path(), "技术笔记").unwrap();
        let file = write_file(tmp.path(), "a.md", b"a");
        add(tmp.path(), file.to_str().unwrap(), Some(&category.id), &NoExtractor).unwrap();
        let file2 = write_file(tmp.path(), "b.md", b"b");
        add(tmp.path(), file2.to_str().unwrap(), None, &NoExtractor).unwrap();

        let docs = list(tmp.path()).unwrap();
        assert_eq!(docs.len(), 2);
        let with_category = docs.iter().find(|d| d.file_name == "a.md").unwrap();
        let without = docs.iter().find(|d| d.file_name == "b.md").unwrap();
        assert_eq!(with_category.category_id.as_deref(), Some(category.id.as_str()));
        assert_eq!(with_category.category_name.as_deref(), Some("技术笔记"));
        assert_eq!(without.category_id, None);
        assert_eq!(without.category_name, None);
    }

    #[test]
    fn test_get_detail_shows_category_name() {
        let tmp = setup();
        let category = crate::categories::create_category(tmp.path(), "会议记录").unwrap();
        let file = write_file(tmp.path(), "a.md", b"a");
        add(tmp.path(), file.to_str().unwrap(), Some(&category.id), &NoExtractor).unwrap();
        let docs = list(tmp.path()).unwrap();

        let detail = get_detail(tmp.path(), &docs[0].id).unwrap();
        assert_eq!(detail.category_id.as_deref(), Some(category.id.as_str()));
        assert_eq!(detail.category_name.as_deref(), Some("会议记录"));
    }

    #[test]
    fn test_set_category_update_and_clear() {
        let tmp = setup();
        let category = crate::categories::create_category(tmp.path(), "技术笔记").unwrap();
        let file = write_file(tmp.path(), "a.md", b"a");
        let id = match add(tmp.path(), file.to_str().unwrap(), None, &NoExtractor).unwrap() {
            AddOutcome::Added { id } => id,
            other => panic!("期望 Added，实际 {other:?}"),
        };

        set_category(tmp.path(), &id, Some(&category.id)).unwrap();
        let detail = get_detail(tmp.path(), &id).unwrap();
        assert_eq!(detail.category_name.as_deref(), Some("技术笔记"));

        set_category(tmp.path(), &id, None).unwrap();
        let detail = get_detail(tmp.path(), &id).unwrap();
        assert_eq!(detail.category_id, None);
        assert_eq!(detail.category_name, None);
    }

    #[test]
    fn test_set_category_missing_document_is_error() {
        let tmp = setup();
        let err = set_category(tmp.path(), "nope", None).unwrap_err();
        assert_eq!(err, "文档不存在: nope");
    }

    #[test]
    fn test_set_category_missing_category_is_error() {
        let tmp = setup();
        let file = write_file(tmp.path(), "a.md", b"a");
        let id = match add(tmp.path(), file.to_str().unwrap(), None, &NoExtractor).unwrap() {
            AddOutcome::Added { id } => id,
            other => panic!("期望 Added，实际 {other:?}"),
        };
        let err = set_category(tmp.path(), &id, Some("nope")).unwrap_err();
        assert_eq!(err, "分类不存在: nope");
    }

    #[test]
    fn test_add_docx_uses_extractor_for_content_and_keeps_original_blob() {
        let tmp = setup();
        let doc = tmp.path().join("a.docx");
        fs::write(&doc, b"PK\x03\x04 fake docx").unwrap();

        let id = match add(tmp.path(), doc.to_str().unwrap(), None, &StubExtractor::new("提取的正文")).unwrap() {
            AddOutcome::Added { id } => id,
            other => panic!("期望 Added，实际 {other:?}"),
        };

        let detail = get_detail(tmp.path(), &id).unwrap();
        assert_eq!(detail.content, "提取的正文");
        assert_eq!(
            detail.mime_type,
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
        );
        // blob 存原始字节
        let blob = blob_path(tmp.path(), &detail.sha1);
        assert_eq!(fs::read(&blob).unwrap(), b"PK\x03\x04 fake docx");
    }

    #[test]
    fn test_add_docx_extractor_failure_is_error() {
        let tmp = setup();
        let doc = tmp.path().join("a.docx");
        fs::write(&doc, b"PK\x03\x04 fake docx").unwrap();

        struct FailingExtractor;

        impl ExtractsText for FailingExtractor {
            fn extract(
                &self,
                _vault: &Path,
                _path: &Path,
                _timeout: std::time::Duration,
            ) -> Result<String, String> {
                Err("文件抽取失败: boom".to_string())
            }
        }

        let error = add(tmp.path(), doc.to_str().unwrap(), None, &FailingExtractor).unwrap_err();
        assert!(error.contains("boom"));
        // 失败不应残留任何行
        assert_eq!(list(tmp.path()).unwrap().len(), 0);
    }
}
