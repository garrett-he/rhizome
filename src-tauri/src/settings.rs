use std::path::Path;

use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;

pub const TABLE_SQL: &str = "CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT
);";

pub const PROMPT_DOCS_COMPILE: &str = "docs_compile";
const DEFAULT_PROMPT_DOCS_COMPILE: &str = include_str!("../prompts/docs_compile.txt");

#[derive(Serialize, Debug, PartialEq)]
pub struct SettingEntry {
    pub key: String,
    pub value: Option<String>,
}

pub fn ensure_table(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(TABLE_SQL)
        .map_err(|e| format!("初始化 settings 表失败: {e}"))
}

/// 内置默认提示词（白名单校验，未知名称返回 None）
pub fn default_prompt(name: &str) -> Option<&'static str> {
    match name {
        PROMPT_DOCS_COMPILE => Some(DEFAULT_PROMPT_DOCS_COMPILE),
        _ => None,
    }
}

pub fn get_all(vault: &Path) -> Result<Vec<SettingEntry>, String> {
    let conn = Connection::open(crate::db::db_path(vault)).map_err(|e| format!("打开数据库失败: {e}"))?;
    ensure_table(&conn)?;
    let mut stmt = conn
        .prepare("SELECT key, value FROM settings ORDER BY key")
        .map_err(|e| format!("查询失败: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(SettingEntry {
                key: row.get(0)?,
                value: row.get(1)?,
            })
        })
        .map_err(|e| format!("查询失败: {e}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("读取结果失败: {e}"))
}

/// 读取单个设置键（不存在为 None）
pub fn get(vault: &Path, key: &str) -> Result<Option<String>, String> {
    let conn = Connection::open(crate::db::db_path(vault)).map_err(|e| format!("打开数据库失败: {e}"))?;
    ensure_table(&conn)?;
    conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| row.get(0))
        .optional()
        .map_err(|e| format!("查询失败: {e}"))
}

/// 写入或删除（value 为 None 时删键）
pub fn set(vault: &Path, key: &str, value: Option<String>) -> Result<(), String> {
    if key.trim().is_empty() {
        return Err("设置键不能为空".to_string());
    }
    let conn = Connection::open(crate::db::db_path(vault)).map_err(|e| format!("打开数据库失败: {e}"))?;
    ensure_table(&conn)?;
    let result = match &value {
        Some(v) => conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            rusqlite::params![key, v],
        ),
        None => conn.execute("DELETE FROM settings WHERE key = ?1", [key]),
    };
    result.map(|_| ()).map_err(|e| format!("写入设置失败: {e}"))
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

    fn find<'a>(entries: &'a [SettingEntry], key: &str) -> Option<&'a str> {
        entries.iter().find(|e| e.key == key).and_then(|e| e.value.as_deref())
    }

    #[test]
    fn test_get_all_empty_vault() {
        let tmp = setup();
        assert_eq!(get_all(tmp.path()).unwrap(), Vec::new());
    }

    #[test]
    fn test_set_and_get_roundtrip() {
        let tmp = setup();
        set(tmp.path(), "embedding_model", Some("BAAI/bge-m3".into())).unwrap();

        let entries = get_all(tmp.path()).unwrap();
        assert_eq!(find(&entries, "embedding_model"), Some("BAAI/bge-m3"));
    }

    #[test]
    fn test_set_overwrites_value() {
        let tmp = setup();
        set(tmp.path(), "k", Some("a".into())).unwrap();
        set(tmp.path(), "k", Some("b".into())).unwrap();

        let entries = get_all(tmp.path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(find(&entries, "k"), Some("b"));
    }

    #[test]
    fn test_set_none_deletes_key() {
        let tmp = setup();
        set(tmp.path(), "k", Some("v".into())).unwrap();
        set(tmp.path(), "k", None).unwrap();

        assert_eq!(get_all(tmp.path()).unwrap(), Vec::new());
    }

    #[test]
    fn test_set_none_on_missing_key_is_ok() {
        let tmp = setup();
        set(tmp.path(), "missing", None).unwrap();
        assert_eq!(get_all(tmp.path()).unwrap(), Vec::new());
    }

    #[test]
    fn test_set_rejects_empty_key() {
        let tmp = setup();
        let err = set(tmp.path(), "   ", Some("v".into())).unwrap_err();
        assert_eq!(err, "设置键不能为空");
    }

    #[test]
    fn test_default_prompt_whitelist() {
        assert!(default_prompt("docs_compile").is_some());
        assert!(default_prompt("unknown").is_none());
    }

    #[test]
    fn test_builtin_prompt_contains_placeholder() {
        let prompt = default_prompt(PROMPT_DOCS_COMPILE).unwrap();
        assert!(prompt.contains("{content}"));
    }
}
