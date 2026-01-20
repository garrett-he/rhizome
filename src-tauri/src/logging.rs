//! 应用日志：追加写 `~/.rhizome/app.log`，用于桌面端问题定位。
//!
//! 保持最小实现（无第三方日志框架）：每行 `RFC3339 [LEVEL] message`。
//! 所有写入失败均静默——日志绝不阻塞业务流程。

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

/// 应用数据目录：`~/.rhizome`（profiles.json 同级）。
pub fn app_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(std::env::temp_dir).join(".rhizome")
}

/// 日志文件路径：`~/.rhizome/app.log`。
pub fn app_log_path() -> PathBuf {
    app_dir().join("app.log")
}

/// 记录一行日志；失败静默。
pub fn log(level: &str, message: &str) {
    let _ = append_to(&app_log_path(), level, message);
}

/// 追加一行日志到指定文件（`dir` 参数化便于测试）。
pub fn append_to(path: &Path, level: &str, message: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let timestamp = OffsetDateTime::now_utc().format(&Rfc3339).unwrap_or_default();
    let line = format!("{timestamp} [{level}] {message}\n");
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(line.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_app_dir_is_home_rhizome() {
        assert!(app_dir().ends_with(".rhizome"));
        assert!(app_log_path().ends_with("app.log"));
    }

    #[test]
    fn test_append_creates_file_and_appends() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("logs").join("app.log");

        append_to(&path, "INFO", "第一条").unwrap();
        append_to(&path, "ERROR", "第二条").unwrap();

        let content = fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].ends_with("[INFO] 第一条"));
        assert!(lines[1].ends_with("[ERROR] 第二条"));
        // 每行以 RFC3339 时间戳开头
        assert!(lines[0].contains("T") && lines[0].contains("Z"));
    }
}
