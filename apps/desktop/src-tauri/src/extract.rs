//! 文件抽取器调用层：spawn `apps/extractor` 的 rhizome-extract CLI（one-shot）。
//!
//! 开发模式通过 `uv run --project apps/extractor rhizome-extract` 调用；
//! 也可用 settings 键 `extractor_path` 指定自定义可执行文件。输出统一
//! UTF-8（CLI 侧已强制 reconfigure）。

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::documents::ExtractsText;
use crate::settings;

pub const SETTING_EXTRACTOR_PATH: &str = "extractor_path";

/// 纯文档类（docx/pptx/xlsx/pdf/html）抽取超时
pub const TIMEOUT_TEXT: Duration = Duration::from_secs(60);
/// 媒体类（OCR/ASR，含首次模型下载）抽取超时
pub const TIMEOUT_MEDIA: Duration = Duration::from_secs(600);

/// CLI 抽取器：实现 documents::ExtractsText，供 add_documents 注入。
pub struct CliExtractor;

impl ExtractsText for CliExtractor {
    fn extract(&self, vault: &Path, path: &Path, timeout: Duration) -> Result<String, String> {
        extract_text(vault, path, timeout)
    }
}

/// 组装 extractor 命令：settings `extractor_path` 优先；其次 dist 分发布局中
/// 可执行文件旁的 bundled extractor；缺省为开发模式 uv run。
pub fn extractor_command(vault: &Path) -> Result<Command, String> {
    if let Some(custom) = settings::get(vault, SETTING_EXTRACTOR_PATH)?
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
    {
        return Ok(custom_command(PathBuf::from(custom)));
    }
    if let Some(bundled) = bundled_extractor() {
        return Ok(custom_command(bundled));
    }
    let project = extractor_project_dir()?;
    let mut command = Command::new("uv");
    command
        .arg("run")
        .arg("--project")
        .arg(project)
        .arg("rhizome-extract")
        // 与 custom_command 一致：防止以 "-" 开头的文件名被当作选项解析
        .arg("--");
    Ok(command)
}

/// dist 分发布局中的 bundled extractor：当前 exe 旁或父目录的
/// `extractor/rhizome-extract(.exe)`。开发模式不存在则返回 None。
fn bundled_extractor() -> Option<PathBuf> {
    let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let parent = exe_dir.parent().map(Path::to_path_buf);
    let mut bases = vec![exe_dir];
    if let Some(parent) = parent {
        bases.push(parent);
    }
    bundled_extractor_in(&bases)
}

/// 在给定基准目录列表中查找 `extractor/rhizome-extract(.exe)`。
fn bundled_extractor_in(bases: &[PathBuf]) -> Option<PathBuf> {
    let base_name = if cfg!(windows) {
        "rhizome-extract.exe"
    } else {
        "rhizome-extract"
    };
    for base in bases {
        let candidate = base.join("extractor").join(base_name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// 开发模式 extractor 项目根（apps/extractor）。
/// CARGO_MANIFEST_DIR = <repo>/apps/desktop/src-tauri，向上三级到仓库根。
fn extractor_project_dir() -> Result<PathBuf, String> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .map(|root| root.join("apps").join("extractor"))
        .ok_or_else(|| "无法定位 apps/extractor 目录".to_string())
}

fn custom_command(program: PathBuf) -> Command {
    let mut command = Command::new(program);
    command.arg("--");
    command
}

/// 调用 extractor 抽取单个文件的文本。
pub fn extract_text(vault: &Path, path: &Path, timeout: Duration) -> Result<String, String> {
    let mut command = extractor_command(vault)?;
    command.arg(path);
    run(command, path, timeout)
}

fn run(mut command: Command, path: &Path, timeout: Duration) -> Result<String, String> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        // 避免桌面应用后台子进程弹出控制台窗口
        use std::os::windows::process::CommandExt as _;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("启动文件抽取器失败: {e}"))?;
    let mut stdout_pipe = child.stdout.take().ok_or("无法读取抽取器输出")?;
    let mut stderr_pipe = child.stderr.take().ok_or("无法读取抽取器错误输出")?;
    let stdout_reader = thread::spawn(move || {
        let mut buffer = Vec::new();
        let _ = stdout_pipe.read_to_end(&mut buffer);
        String::from_utf8_lossy(&buffer).into_owned()
    });
    let stderr_reader = thread::spawn(move || {
        let mut buffer = Vec::new();
        let _ = stderr_pipe.read_to_end(&mut buffer);
        String::from_utf8_lossy(&buffer).into_owned()
    });

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("文件抽取超时: {path:?}"));
            }
            Ok(None) => thread::sleep(Duration::from_millis(50)),
            Err(e) => return Err(format!("等待文件抽取器失败: {e}")),
        }
    };

    let stdout = stdout_reader.join().map_err(|_| "读取抽取器输出失败".to_string())?;
    let stderr = stderr_reader.join().map_err(|_| "读取抽取器错误输出失败".to_string())?;
    if !status.success() {
        let message = stderr.trim();
        let detail = if message.is_empty() {
            format!("退出码 {status}")
        } else {
            message.to_string()
        };
        return Err(format!("文件抽取失败: {path:?}: {detail}"));
    }
    Ok(stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extractor_project_dir_points_to_apps_extractor() {
        let dir = extractor_project_dir().unwrap();
        assert!(dir.ends_with("apps/extractor") || dir.ends_with("apps\\extractor"));
        assert!(dir.join("pyproject.toml").is_file());
    }

    #[test]
    fn test_extractor_command_uses_custom_path_from_settings() {
        let tmp = tempfile::TempDir::new().unwrap();
        crate::db::ensure_db(tmp.path()).unwrap();
        crate::settings::set(
            tmp.path(),
            SETTING_EXTRACTOR_PATH,
            Some("C:/tools/rhizome-extract.exe".to_string()),
        )
        .unwrap();

        let command = extractor_command(tmp.path()).unwrap();

        let program = command.get_program().to_string_lossy().into_owned();
        assert!(program.ends_with("rhizome-extract.exe"));
        // 自定义可执行文件第一个参数是占位符 "--"，文件路径由调用方追加
        let args: Vec<String> = command
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args, vec!["--"]);
    }

    #[test]
    fn test_bundled_extractor_in_finds_exe_layout() {
        let tmp = tempfile::TempDir::new().unwrap();
        let bundled = tmp.path().join("extractor");
        std::fs::create_dir_all(&bundled).unwrap();
        let exe = bundled.join(if cfg!(windows) {
            "rhizome-extract.exe"
        } else {
            "rhizome-extract"
        });
        std::fs::write(&exe, b"stub").unwrap();

        let found = bundled_extractor_in(&[tmp.path().to_path_buf()]).unwrap();
        assert_eq!(found, exe);
    }

    #[test]
    fn test_bundled_extractor_in_returns_none_when_missing() {
        let tmp = tempfile::TempDir::new().unwrap();
        assert!(bundled_extractor_in(&[tmp.path().to_path_buf()]).is_none());
        assert!(bundled_extractor_in(&[]).is_none());
    }

    #[test]
    fn test_bundled_extractor_is_none_in_dev_mode() {
        // 测试环境（target/debug/deps）没有 dist 布局，bundled 定位应返回 None，
        // extractor_command 会继续走 dev 模式 uv run。
        assert!(bundled_extractor().is_none());
    }

    #[test]
    fn test_extractor_command_dev_mode_uses_uv_run() {
        let tmp = tempfile::TempDir::new().unwrap();
        crate::db::ensure_db(tmp.path()).unwrap();

        let command = extractor_command(tmp.path()).unwrap();

        assert_eq!(command.get_program().to_string_lossy(), "uv");
        let args: Vec<String> = command
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args[0], "run");
        assert_eq!(args[1], "--project");
    }

    #[cfg(windows)]
    #[test]
    fn test_run_returns_stdout_on_success() {
        let mut command = Command::new("cmd");
        command.args(["/C", "echo", "hello-extract"]);
        let output = run(command, Path::new("dummy"), TIMEOUT_TEXT).unwrap();
        assert!(output.contains("hello-extract"));
    }

    #[cfg(windows)]
    #[test]
    fn test_run_reports_stderr_on_failure() {
        let mut command = Command::new("cmd");
        command.args(["/C", "echo boom >&2 & exit /b 1"]);
        let error = run(command, Path::new("dummy"), TIMEOUT_TEXT).unwrap_err();
        assert!(error.contains("boom"));
        assert!(error.contains("dummy"));
    }

    #[cfg(windows)]
    #[test]
    fn test_run_kills_process_on_timeout() {
        let mut command = Command::new("cmd");
        command.args(["/C", "ping -n 5 127.0.0.1"]);
        let error = run(command, Path::new("dummy"), Duration::from_millis(500)).unwrap_err();
        assert!(error.contains("超时"));
    }
}
