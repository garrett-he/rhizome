use std::{
    fs,
    path::{Path, PathBuf},
};

use rusqlite::Connection;
use serde::Serialize;

pub const DB_FILE: &str = "rhizome.db";

#[derive(Serialize, Debug, PartialEq)]
pub struct CategoryStat {
    pub name: String,
    pub count: u64,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct Overview {
    pub total_documents: u64,
    pub indexed_documents: u64,
    pub pending_documents: u64,
    pub failed_documents: u64,
    pub storage_bytes: u64,
    pub categories: Vec<CategoryStat>,
}

pub fn db_path(vault: &Path) -> PathBuf {
    vault.join(DB_FILE)
}

/// 打开/创建 {vault}/rhizome.db 并确保 settings 与 documents 表存在（幂等）。
pub fn ensure_db(vault: &Path) -> Result<(), String> {
    let conn = Connection::open(db_path(vault)).map_err(|e| format!("打开数据库失败: {e}"))?;
    crate::settings::ensure_table(&conn)?;
    crate::documents::ensure_table(&conn)?;
    Ok(())
}

/// 总览统计。文档计数来自 documents 表真实查询（failed 恒 0，最小表无 last_error 列）；
/// 存储占用为 vault 目录的实时大小（含 rhizome.db 与 blobs）。
pub fn get_overview(vault: &Path) -> Result<Overview, String> {
    let conn = Connection::open(db_path(vault)).map_err(|e| format!("打开数据库失败: {e}"))?;
    let (total_documents, indexed_documents, pending_documents) = crate::documents::counts(&conn);
    Ok(Overview {
        total_documents,
        indexed_documents,
        pending_documents,
        failed_documents: 0,
        storage_bytes: dir_size(vault),
        categories: Vec::new(),
    })
}

fn dir_size(path: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    let mut total = 0;
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            total += dir_size(&entry.path());
        } else if let Ok(meta) = entry.metadata() {
            total += meta.len();
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn settings_table_exists(vault: &Path) -> bool {
        let conn = Connection::open(db_path(vault)).unwrap();
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'settings'")
            .unwrap();
        stmt.exists([]).unwrap()
    }

    #[test]
    fn test_ensure_db_creates_file_and_settings_table() {
        let tmp = TempDir::new().unwrap();
        ensure_db(tmp.path()).expect("ensure_db 失败");

        assert!(db_path(tmp.path()).exists());
        assert!(settings_table_exists(tmp.path()));
    }

    #[test]
    fn test_ensure_db_is_idempotent() {
        let tmp = TempDir::new().unwrap();
        ensure_db(tmp.path()).unwrap();
        ensure_db(tmp.path()).unwrap();

        assert!(settings_table_exists(tmp.path()));
    }

    #[test]
    fn test_settings_table_accepts_kv_rows() {
        let tmp = TempDir::new().unwrap();
        ensure_db(tmp.path()).unwrap();

        let conn = Connection::open(db_path(tmp.path())).unwrap();
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('llm_model', 'openai/gpt-5')",
            [],
        )
        .unwrap();
        let value: String = conn
            .query_row("SELECT value FROM settings WHERE key = 'llm_model'", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(value, "openai/gpt-5");
    }

    #[test]
    fn test_dir_size_sums_files_recursively() {
        let tmp = TempDir::new().unwrap();
        let nested = tmp.path().join("nested");
        fs::create_dir_all(&nested).unwrap();
        fs::write(tmp.path().join("a.txt"), [0u8; 100]).unwrap();
        fs::write(nested.join("b.txt"), [0u8; 50]).unwrap();

        assert_eq!(dir_size(tmp.path()), 150);
    }

    #[test]
    fn test_dir_size_missing_path_is_zero() {
        assert_eq!(dir_size(Path::new("Z:/definitely/missing")), 0);
    }

    #[test]
    fn test_get_overview_returns_zero_counts_and_real_storage() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("data.bin"), [0u8; 256]).unwrap();

        let overview = get_overview(tmp.path()).unwrap();
        assert_eq!(overview.total_documents, 0);
        assert_eq!(overview.indexed_documents, 0);
        assert_eq!(overview.pending_documents, 0);
        assert_eq!(overview.failed_documents, 0);
        assert_eq!(overview.categories, Vec::new());
        assert_eq!(overview.storage_bytes, 256);
    }
}
