//! 分类管理：categories 表与 CRUD 服务（移植 deprecated/src/rhizome/core/categories.py）。

use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

pub const CATEGORIES_TABLE_SQL: &str = "
CREATE TABLE IF NOT EXISTS categories (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);";

/// 分类列表条目：名称 + 关联文档数（总览「分类分布」与分类管理页共用）。
#[derive(Serialize, Debug, PartialEq)]
pub struct CategorySummary {
    pub id: String,
    pub name: String,
    pub document_count: u64,
}

/// 删除分类的结果：被 detach（置为未分类）的文档数。
#[derive(Serialize, Debug, PartialEq)]
pub struct DeleteOutcome {
    pub detached: u64,
}

fn now_rfc3339() -> String {
    OffsetDateTime::now_utc().format(&Rfc3339).unwrap_or_default()
}

pub fn ensure_table(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(CATEGORIES_TABLE_SQL)
        .map_err(|e| format!("初始化 categories 表失败: {e}"))
}

fn open_conn(vault: &Path) -> Result<Connection, String> {
    let conn = Connection::open(crate::db::db_path(vault)).map_err(|e| format!("打开数据库失败: {e}"))?;
    ensure_table(&conn)?;
    Ok(conn)
}

fn normalize_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("分类名不能为空".to_string());
    }
    Ok(name.to_string())
}

/// 按 id 查单个分类（含文档计数）；不存在返回 Err。
fn summary_by_id(conn: &Connection, id: &str) -> Result<CategorySummary, String> {
    conn.query_row(
        "SELECT c.id, c.name, COUNT(d.id)
         FROM categories c LEFT JOIN documents d ON d.category_id = c.id
         WHERE c.id = ?1
         GROUP BY c.id, c.name",
        [id],
        |row| {
            Ok(CategorySummary {
                id: row.get(0)?,
                name: row.get(1)?,
                document_count: row.get::<_, i64>(2)?.max(0) as u64,
            })
        },
    )
    .optional()
    .map_err(|e| format!("查询失败: {e}"))?
    .ok_or_else(|| format!("分类不存在: {id}"))
}

/// 创建分类；名称唯一，重名返回 Err。
pub fn create_category(vault: &Path, name: &str) -> Result<CategorySummary, String> {
    let name = normalize_name(name)?;
    let conn = open_conn(vault)?;
    if find_id_by_name(&conn, &name)?.is_some() {
        return Err(format!("分类 '{name}' 已存在"));
    }
    let id = Uuid::new_v4().to_string().replace('-', "");
    let now = now_rfc3339();
    conn.execute(
        "INSERT INTO categories (id, name, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
        params![id, name, now],
    )
    .map_err(|e| format!("写入数据库失败: {e}"))?;
    summary_by_id(&conn, &id)
}

/// 全部分类（按名称排序，附带文档计数，一次 JOIN 聚合查询）。
pub fn list_categories(vault: &Path) -> Result<Vec<CategorySummary>, String> {
    let conn = open_conn(vault)?;
    let mut stmt = conn
        .prepare(
            "SELECT c.id, c.name, COUNT(d.id)
             FROM categories c LEFT JOIN documents d ON d.category_id = c.id
             GROUP BY c.id, c.name
             ORDER BY c.name",
        )
        .map_err(|e| format!("查询失败: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(CategorySummary {
                id: row.get(0)?,
                name: row.get(1)?,
                document_count: row.get::<_, i64>(2)?.max(0) as u64,
            })
        })
        .map_err(|e| format!("查询失败: {e}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("读取结果失败: {e}"))
}

fn find_id_by_name(conn: &Connection, name: &str) -> Result<Option<String>, String> {
    conn.query_row("SELECT id FROM categories WHERE name = ?1", [name], |row| row.get(0))
        .optional()
        .map_err(|e| format!("查询失败: {e}"))
}

/// 重命名分类；新名与其他分类冲突时返回 Err，文档关联保持不变。
pub fn rename_category(vault: &Path, id: &str, new_name: &str) -> Result<CategorySummary, String> {
    let new_name = normalize_name(new_name)?;
    let conn = open_conn(vault)?;
    let exists = conn
        .query_row("SELECT 1 FROM categories WHERE id = ?1", [id], |_| Ok(()))
        .optional()
        .map_err(|e| format!("查询失败: {e}"))?;
    if exists.is_none() {
        return Err(format!("分类不存在: {id}"));
    }
    if let Some(clash_id) = find_id_by_name(&conn, &new_name)? {
        if clash_id != id {
            return Err(format!("分类 '{new_name}' 已存在"));
        }
    }
    conn.execute(
        "UPDATE categories SET name = ?1, updated_at = ?2 WHERE id = ?3",
        params![new_name, now_rfc3339(), id],
    )
    .map_err(|e| format!("写入数据库失败: {e}"))?;
    summary_by_id(&conn, id)
}

/// 删除分类并把关联文档 detach（category_id 置空）；返回 detach 数量。
pub fn delete_category(vault: &Path, id: &str) -> Result<DeleteOutcome, String> {
    let mut conn = open_conn(vault)?;
    let tx = conn.transaction().map_err(|e| format!("开启事务失败: {e}"))?;
    let exists = tx
        .query_row("SELECT 1 FROM categories WHERE id = ?1", [id], |_| Ok(()))
        .optional()
        .map_err(|e| format!("查询失败: {e}"))?;
    if exists.is_none() {
        return Err(format!("分类不存在: {id}"));
    }
    let detached = tx
        .execute("UPDATE documents SET category_id = NULL WHERE category_id = ?1", [id])
        .map_err(|e| format!("解除文档关联失败: {e}"))? as u64;
    tx.execute("DELETE FROM categories WHERE id = ?1", [id])
        .map_err(|e| format!("删除分类失败: {e}"))?;
    tx.commit().map_err(|e| format!("提交事务失败: {e}"))?;
    Ok(DeleteOutcome { detached })
}

/// 按名称解析分类，不存在时自动创建（添加文档指定分类用）。
pub fn ensure_category(vault: &Path, name: &str) -> Result<String, String> {
    let name = normalize_name(name)?;
    let conn = open_conn(vault)?;
    if let Some(id) = find_id_by_name(&conn, &name)? {
        return Ok(id);
    }
    let id = Uuid::new_v4().to_string().replace('-', "");
    let now = now_rfc3339();
    conn.execute(
        "INSERT INTO categories (id, name, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
        params![id, name, now],
    )
    .map_err(|e| format!("写入数据库失败: {e}"))?;
    Ok(id)
}

/// 分类分布统计（总览「分类分布」）：名称 + 文档数，按名称排序；
/// categories 表缺失时返回空（防御旧库未迁移）。
pub fn stats(conn: &Connection) -> Result<Vec<crate::db::CategoryStat>, String> {
    let table_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'categories'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map(|n| n > 0)
        .unwrap_or(false);
    if !table_exists {
        return Ok(Vec::new());
    }
    let mut stmt = conn
        .prepare(
            "SELECT c.name, COUNT(d.id)
             FROM categories c LEFT JOIN documents d ON d.category_id = c.id
             GROUP BY c.id, c.name
             ORDER BY c.name",
        )
        .map_err(|e| format!("查询失败: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(crate::db::CategoryStat {
                name: row.get(0)?,
                count: row.get::<_, i64>(1)?.max(0) as u64,
            })
        })
        .map_err(|e| format!("查询失败: {e}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("读取结果失败: {e}"))
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

    fn insert_document(vault: &Path, id: &str, category_id: Option<&str>) {
        let conn = Connection::open(db_path_of(vault)).unwrap();
        conn.execute(
            "INSERT INTO documents (id, file_name, mime_type, type, sha1, size, content, status, created_at, updated_at, category_id)
             VALUES (?1, ?1, 'text/markdown', 'plaintext', ?2, 1, 'c', 'raw', 't', 't', ?3)",
            params![id, format!("sha-{id}"), category_id],
        )
        .unwrap();
    }

    fn db_path_of(vault: &Path) -> std::path::PathBuf {
        crate::db::db_path(vault)
    }

    #[test]
    fn test_create_and_list_sorted_with_counts() {
        let tmp = setup();
        create_category(tmp.path(), "技术笔记").unwrap();
        create_category(tmp.path(), "会议记录").unwrap();
        insert_document(tmp.path(), "d1", None);
        let tech = list_categories(tmp.path()).unwrap();
        assert_eq!(tech.len(), 2);
        assert_eq!(tech[0].name, "会议记录");
        assert_eq!(tech[0].document_count, 0);
        assert_eq!(tech[1].name, "技术笔记");
        assert_eq!(tech[1].document_count, 0);
    }

    #[test]
    fn test_create_duplicate_name_is_error() {
        let tmp = setup();
        create_category(tmp.path(), "技术笔记").unwrap();
        let err = create_category(tmp.path(), "技术笔记").unwrap_err();
        assert_eq!(err, "分类 '技术笔记' 已存在");
    }

    #[test]
    fn test_create_empty_name_is_error() {
        let tmp = setup();
        assert_eq!(create_category(tmp.path(), "   ").unwrap_err(), "分类名不能为空");
    }

    #[test]
    fn test_create_trims_name() {
        let tmp = setup();
        let category = create_category(tmp.path(), "  技术笔记  ").unwrap();
        assert_eq!(category.name, "技术笔记");
    }

    #[test]
    fn test_rename_updates_name_and_keeps_count() {
        let tmp = setup();
        let category = create_category(tmp.path(), "旧名").unwrap();
        insert_document(tmp.path(), "d1", Some(&category.id));
        insert_document(tmp.path(), "d2", Some(&category.id));

        let renamed = rename_category(tmp.path(), &category.id, "新名").unwrap();

        assert_eq!(renamed.name, "新名");
        assert_eq!(renamed.document_count, 2);
    }

    #[test]
    fn test_rename_clash_is_error_and_same_name_self_ok() {
        let tmp = setup();
        let a = create_category(tmp.path(), "A").unwrap();
        let b = create_category(tmp.path(), "B").unwrap();
        assert_eq!(rename_category(tmp.path(), &a.id, "B").unwrap_err(), "分类 'B' 已存在");
        // 改成自己的名字是允许的
        rename_category(tmp.path(), &b.id, "B").unwrap();
    }

    #[test]
    fn test_rename_missing_is_error() {
        let tmp = setup();
        let err = rename_category(tmp.path(), "nope", "新名").unwrap_err();
        assert_eq!(err, "分类不存在: nope");
    }

    #[test]
    fn test_delete_detaches_documents_and_reports_count() {
        let tmp = setup();
        let category = create_category(tmp.path(), "技术笔记").unwrap();
        insert_document(tmp.path(), "d1", Some(&category.id));
        insert_document(tmp.path(), "d2", Some(&category.id));
        insert_document(tmp.path(), "d3", None);

        let outcome = delete_category(tmp.path(), &category.id).unwrap();

        assert_eq!(outcome.detached, 2);
        assert!(list_categories(tmp.path()).unwrap().is_empty());
        let conn = Connection::open(db_path_of(tmp.path())).unwrap();
        let orphan: Option<String> = conn
            .query_row("SELECT category_id FROM documents WHERE id = 'd1'", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(orphan, None);
    }

    #[test]
    fn test_delete_missing_is_error() {
        let tmp = setup();
        let err = delete_category(tmp.path(), "nope").unwrap_err();
        assert_eq!(err, "分类不存在: nope");
    }

    #[test]
    fn test_ensure_category_creates_once_then_resolves() {
        let tmp = setup();
        let id = ensure_category(tmp.path(), "技术笔记").unwrap();
        let again = ensure_category(tmp.path(), "  技术笔记  ").unwrap();
        assert_eq!(id, again);
        let categories = list_categories(tmp.path()).unwrap();
        assert_eq!(categories.len(), 1);
        assert_eq!(categories[0].id, id);
    }

    #[test]
    fn test_ensure_category_empty_name_is_error() {
        let tmp = setup();
        assert_eq!(ensure_category(tmp.path(), "").unwrap_err(), "分类名不能为空");
    }

    #[test]
    fn test_stats_returns_sorted_counts() {
        let tmp = setup();
        let a = create_category(tmp.path(), "A").unwrap();
        let b = create_category(tmp.path(), "B").unwrap();
        insert_document(tmp.path(), "d1", Some(&a.id));
        insert_document(tmp.path(), "d2", Some(&a.id));
        insert_document(tmp.path(), "d3", Some(&b.id));

        let conn = Connection::open(db_path_of(tmp.path())).unwrap();
        let categories = stats(&conn).unwrap();
        assert_eq!(
            categories,
            vec![
                crate::db::CategoryStat {
                    name: "A".to_string(),
                    count: 2,
                },
                crate::db::CategoryStat {
                    name: "B".to_string(),
                    count: 1,
                },
            ]
        );
    }

    #[test]
    fn test_ensure_table_is_idempotent() {
        let tmp = setup();
        let conn = Connection::open(db_path_of(tmp.path())).unwrap();
        ensure_table(&conn).unwrap();
        ensure_table(&conn).unwrap();
    }
}
