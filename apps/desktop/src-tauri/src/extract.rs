//! 文件抽取器调用层：spawn any2txt 的 `--stdout` 机器接口（one-shot）。
//!
//! any2txt 只把抽取文本写到 stdout，诊断信息全部走 stderr；图片等资源用
//! `--assets drop` 丢弃，避免生成无人引用的临时文件。开发模式通过
//! `uv run --project <同级 any2txt 仓库> any2txt` 调用；dist 布局在可执行
//! 文件旁查找 `any2txt/any2txt(.exe)`；settings 键 `any2txt_path` 可覆盖。

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::documents::ExtractsText;
use crate::settings;

pub const SETTING_ANY2TXT_PATH: &str = "any2txt_path";

/// 纯文档类（docx/pptx/xlsx/pdf/html）抽取超时
pub const TIMEOUT_TEXT: Duration = Duration::from_secs(60);
/// 媒体类（OCR/ASR，含首次模型下载）抽取超时
pub const TIMEOUT_MEDIA: Duration = Duration::from_secs(600);

/// any2txt 机器接口参数：只输出文本、丢弃资源，`--` 结束选项解析。
const ANY2TXT_ARGS: [&str; 4] = ["--stdout", "--assets", "drop", "--"];

const HF_ENDPOINT_DEFAULT: &str = "https://hf-mirror.com";

/// any2txt 抽取器：实现 documents::ExtractsText，供 add_documents 注入。
pub struct Any2txtExtractor;

impl ExtractsText for Any2txtExtractor {
    fn extract(&self, vault: &Path, path: &Path, timeout: Duration) -> Result<String, String> {
        extract_text(vault, path, timeout)
    }
}

/// 组装 any2txt 命令：settings `any2txt_path` 优先；其次 dist 分发布局中
/// 可执行文件旁的 bundled any2txt；缺省为开发模式 uv run 同级仓库。
pub fn extractor_command(vault: &Path) -> Result<Command, String> {
    let mut command = if let Some(custom) = settings::get(vault, SETTING_ANY2TXT_PATH)?
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
    {
        program_command(PathBuf::from(custom))
    } else if let Some(bundled) = bundled_any2txt() {
        program_command(bundled)
    } else {
        dev_command()?
    };
    configure_hf_environment(&mut command);
    Ok(command)
}

/// 源码运行（uv run）时让 faster-whisper 与 rhizome 共用模型缓存和镜像。
fn configure_hf_environment(command: &mut Command) {
    let endpoint = std::env::var("HF_ENDPOINT").unwrap_or_else(|_| HF_ENDPOINT_DEFAULT.to_string());
    command.env("HF_ENDPOINT", endpoint);
    if std::env::var_os("HF_HOME").is_none() {
        command.env("HF_HOME", crate::logging::app_dir().join("models").join("huggingface"));
    }
}

/// 直接执行 any2txt 可执行文件的命令。
fn program_command(program: PathBuf) -> Command {
    let mut command = Command::new(program);
    command.args(ANY2TXT_ARGS);
    command
}

/// 开发模式：通过 uv 运行同级 any2txt 仓库的 CLI。
fn dev_command() -> Result<Command, String> {
    let mut command = Command::new("uv");
    command
        .arg("run")
        .arg("--project")
        .arg(any2txt_project_dir()?)
        .arg("any2txt")
        .args(ANY2TXT_ARGS);
    Ok(command)
}

/// dist 分发布局中的 bundled any2txt：当前 exe 旁或父目录的
/// `any2txt/any2txt(.exe)`。开发模式不存在则返回 None。
fn bundled_any2txt() -> Option<PathBuf> {
    let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let parent = exe_dir.parent().map(Path::to_path_buf);
    let mut bases = vec![exe_dir];
    if let Some(parent) = parent {
        bases.push(parent);
    }
    bundled_any2txt_in(&bases)
}

/// 在给定基准目录列表中查找 `any2txt/any2txt(.exe)`。
fn bundled_any2txt_in(bases: &[PathBuf]) -> Option<PathBuf> {
    let base_name = if cfg!(windows) { "any2txt.exe" } else { "any2txt" };
    bases
        .iter()
        .map(|base| base.join("any2txt").join(base_name))
        .find(|candidate| candidate.is_file())
}

/// 开发模式 any2txt 仓库根：与本仓库（rhizome）同级。
/// CARGO_MANIFEST_DIR = <repo>/apps/desktop/src-tauri，向上四级到仓库父目录。
fn any2txt_project_dir() -> Result<PathBuf, String> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .and_then(Path::parent)
        .map(|projects| projects.join("any2txt"))
        .ok_or_else(|| "无法定位 any2txt 仓库（应与 rhizome 位于同一父目录）".to_string())
}

/// 调用 any2txt 抽取单个文件的文本。
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
    let mut child = command.spawn().map_err(|e| format!("启动 any2txt 失败: {e}"))?;
    let mut stdout_pipe = child.stdout.take().ok_or("无法读取 any2txt 输出")?;
    let mut stderr_pipe = child.stderr.take().ok_or("无法读取 any2txt 错误输出")?;
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
            Err(e) => return Err(format!("等待 any2txt 失败: {e}")),
        }
    };

    let stdout = stdout_reader.join().map_err(|_| "读取 any2txt 输出失败".to_string())?;
    let stderr = stderr_reader
        .join()
        .map_err(|_| "读取 any2txt 错误输出失败".to_string())?;
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
    fn test_any2txt_project_dir_is_sibling_repo() {
        let dir = any2txt_project_dir().unwrap();
        assert_eq!(dir.file_name().unwrap(), "any2txt");
        assert!(!dir.to_string_lossy().contains("apps"));
    }

    #[test]
    fn test_extractor_command_uses_custom_path_from_settings() {
        let tmp = tempfile::TempDir::new().unwrap();
        crate::db::ensure_db(tmp.path()).unwrap();
        crate::settings::set(
            tmp.path(),
            SETTING_ANY2TXT_PATH,
            Some("C:/tools/any2txt.exe".to_string()),
        )
        .unwrap();

        let command = extractor_command(tmp.path()).unwrap();

        let program = command.get_program().to_string_lossy().into_owned();
        assert!(program.ends_with("any2txt.exe"));
        let args: Vec<String> = command.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
        assert_eq!(args.join(" "), ANY2TXT_ARGS.join(" "));
    }

    #[test]
    fn test_bundled_any2txt_in_finds_exe_layout() {
        let tmp = tempfile::TempDir::new().unwrap();
        let bundled = tmp.path().join("any2txt");
        std::fs::create_dir_all(&bundled).unwrap();
        let exe = bundled.join(if cfg!(windows) { "any2txt.exe" } else { "any2txt" });
        std::fs::write(&exe, b"stub").unwrap();

        let found = bundled_any2txt_in(&[tmp.path().to_path_buf()]).unwrap();
        assert_eq!(found, exe);
    }

    #[test]
    fn test_bundled_any2txt_in_returns_none_when_missing() {
        let tmp = tempfile::TempDir::new().unwrap();
        assert!(bundled_any2txt_in(&[tmp.path().to_path_buf()]).is_none());
        assert!(bundled_any2txt_in(&[]).is_none());
    }

    #[test]
    fn test_bundled_any2txt_is_none_in_dev_mode() {
        // 测试环境（target/debug/deps）没有 dist 布局，bundled 定位应返回 None，
        // extractor_command 会继续走开发模式 uv run。
        assert!(bundled_any2txt().is_none());
    }

    #[test]
    fn test_extractor_command_dev_mode_uses_uv_run() {
        let tmp = tempfile::TempDir::new().unwrap();
        crate::db::ensure_db(tmp.path()).unwrap();

        let command = extractor_command(tmp.path()).unwrap();

        assert_eq!(command.get_program().to_string_lossy(), "uv");
        let args: Vec<String> = command.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
        assert_eq!(args[0], "run");
        assert_eq!(args[1], "--project");
        assert_eq!(args[3], "any2txt");
        assert_eq!(args[4..].join(" "), ANY2TXT_ARGS.join(" "));
    }

    #[test]
    fn test_any2txt_args_never_embed_assets() {
        // Extracted text is sent to the LLM for compilation: inlined image data
        // URIs (`--assets embed`) would make prompts enormous, so assets must be dropped.
        assert_eq!(ANY2TXT_ARGS, ["--stdout", "--assets", "drop", "--"]);
        assert!(!ANY2TXT_ARGS.contains(&"embed"));
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
        command.args(["/C", "ping", "-n", "5", "127.0.0.1"]);
        let error = run(command, Path::new("dummy"), Duration::from_millis(500)).unwrap_err();
        assert!(error.contains("超时"));
    }
}
