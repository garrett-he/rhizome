//! 任务历史：jobs 表与批量处理记录服务。

use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

pub const JOBS_TABLE_SQL: &str = "
CREATE TABLE IF NOT EXISTS jobs (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    total INTEGER NOT NULL,
    succeeded INTEGER NOT NULL,
    failed INTEGER NOT NULL,
    status TEXT NOT NULL,
    started_at TEXT NOT NULL,
    finished_at TEXT
);";

/// 一次批量处理的历史记录（任务中心「历史任务」展示用）。
#[derive(Serialize, Debug, PartialEq, Clone)]
pub struct JobRecord {
    pub id: String,
    pub kind: String,
    pub total: u64,
    pub succeeded: u64,
    pub failed: u64,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
}

fn now_rfc3339() -> String {
    OffsetDateTime::now_utc().format(&Rfc3339).unwrap_or_default()
}

pub fn ensure_table(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(JOBS_TABLE_SQL)
        .map_err(|e| format!("初始化 jobs 表失败: {e}"))
}

fn open_conn(vault: &Path) -> Result<Connection, String> {
    let conn = crate::db::open_conn(vault)?;
    ensure_table(&conn)?;
    Ok(conn)
}

/// 记录一次批量处理开始（status=running，成败计数为 0）。
pub fn job_start(vault: &Path, kind: &str, total: u64) -> Result<JobRecord, String> {
    let conn = open_conn(vault)?;
    let record = JobRecord {
        id: Uuid::new_v4().to_string().replace('-', ""),
        kind: kind.to_string(),
        total,
        succeeded: 0,
        failed: 0,
        status: "running".to_string(),
        started_at: now_rfc3339(),
        finished_at: None,
    };
    conn.execute(
        "INSERT INTO jobs (id, kind, total, succeeded, failed, status, started_at, finished_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL)",
        params![
            record.id,
            record.kind,
            record.total as i64,
            0,
            0,
            record.status,
            record.started_at
        ],
    )
    .map_err(|e| format!("写入数据库失败: {e}"))?;
    Ok(record)
}

/// 记录批量处理结束（status=completed，写入成败计数与完成时间）；任务不存在返回 Err。
pub fn job_finish(vault: &Path, job_id: &str, succeeded: u64, failed: u64) -> Result<JobRecord, String> {
    let conn = open_conn(vault)?;
    let finished_at = now_rfc3339();
    let updated = conn
        .execute(
            "UPDATE jobs SET succeeded = ?1, failed = ?2, status = 'completed', finished_at = ?3 WHERE id = ?4",
            params![succeeded as i64, failed as i64, finished_at, job_id],
        )
        .map_err(|e| format!("写入数据库失败: {e}"))?;
    if updated == 0 {
        return Err(format!("任务不存在: {job_id}"));
    }
    get_job(&conn, job_id)
}

/// 最近的任务历史（started_at 降序）。
pub fn list_jobs(vault: &Path, limit: u32) -> Result<Vec<JobRecord>, String> {
    let conn = open_conn(vault)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, kind, total, succeeded, failed, status, started_at, finished_at
             FROM jobs ORDER BY started_at DESC LIMIT ?1",
        )
        .map_err(|e| format!("查询失败: {e}"))?;
    let rows = stmt
        .query_map([limit], |row| {
            Ok(JobRecord {
                id: row.get(0)?,
                kind: row.get(1)?,
                total: row.get::<_, i64>(2)?.max(0) as u64,
                succeeded: row.get::<_, i64>(3)?.max(0) as u64,
                failed: row.get::<_, i64>(4)?.max(0) as u64,
                status: row.get(5)?,
                started_at: row.get(6)?,
                finished_at: row.get(7)?,
            })
        })
        .map_err(|e| format!("查询失败: {e}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("读取结果失败: {e}"))
}

fn get_job(conn: &Connection, job_id: &str) -> Result<JobRecord, String> {
    conn.query_row(
        "SELECT id, kind, total, succeeded, failed, status, started_at, finished_at
         FROM jobs WHERE id = ?1",
        [job_id],
        |row| {
            Ok(JobRecord {
                id: row.get(0)?,
                kind: row.get(1)?,
                total: row.get::<_, i64>(2)?.max(0) as u64,
                succeeded: row.get::<_, i64>(3)?.max(0) as u64,
                failed: row.get::<_, i64>(4)?.max(0) as u64,
                status: row.get(5)?,
                started_at: row.get(6)?,
                finished_at: row.get(7)?,
            })
        },
    )
    .optional()
    .map_err(|e| format!("查询失败: {e}"))?
    .ok_or_else(|| format!("任务不存在: {job_id}"))
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

    #[test]
    fn test_job_start_creates_running_record() {
        let tmp = setup();
        let job = job_start(tmp.path(), "all", 3).unwrap();
        assert_eq!(job.kind, "all");
        assert_eq!(job.total, 3);
        assert_eq!(job.succeeded, 0);
        assert_eq!(job.failed, 0);
        assert_eq!(job.status, "running");
        assert_eq!(job.finished_at, None);
    }

    #[test]
    fn test_job_finish_completes_record() {
        let tmp = setup();
        let job = job_start(tmp.path(), "compile", 2).unwrap();
        let finished = job_finish(tmp.path(), &job.id, 1, 1).unwrap();
        assert_eq!(finished.status, "completed");
        assert_eq!(finished.succeeded, 1);
        assert_eq!(finished.failed, 1);
        assert!(finished.finished_at.is_some());
    }

    #[test]
    fn test_job_finish_missing_is_error() {
        let tmp = setup();
        let err = job_finish(tmp.path(), "nope", 1, 0).unwrap_err();
        assert_eq!(err, "任务不存在: nope");
    }

    #[test]
    fn test_list_jobs_orders_newest_first_with_limit() {
        let tmp = setup();
        let first = job_start(tmp.path(), "compile", 1).unwrap();
        let second = job_start(tmp.path(), "all", 2).unwrap();
        job_finish(tmp.path(), &first.id, 1, 0).unwrap();

        let jobs = list_jobs(tmp.path(), 10).unwrap();
        assert_eq!(jobs.len(), 2);
        assert_eq!(jobs[0].id, second.id);
        assert_eq!(jobs[0].status, "running");
        assert_eq!(jobs[1].id, first.id);
        assert_eq!(jobs[1].status, "completed");

        let limited = list_jobs(tmp.path(), 1).unwrap();
        assert_eq!(limited.len(), 1);
        assert_eq!(limited[0].id, second.id);
    }

    #[test]
    fn test_ensure_table_is_idempotent() {
        let tmp = setup();
        let conn = Connection::open(crate::db::db_path(tmp.path())).unwrap();
        ensure_table(&conn).unwrap();
        ensure_table(&conn).unwrap();
    }
}
