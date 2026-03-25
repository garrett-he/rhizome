"""One-shot text extraction CLI.

``rhizome-extract <file> [options]`` writes the extracted text (Markdown
for documents, plain OCR text for images, SRT subtitle text for
audio/video) to stdout. Errors go to stderr with a non-zero exit code:
2 for unsupported file types, 1 for extraction failures.
"""

import argparse
import os
import sys
from pathlib import Path

_DEPS_DIRNAME = "deps"

EXIT_OK = 0
EXIT_FAILED = 1
EXIT_UNSUPPORTED = 2


def _bundled_hf_home(bases: list[Path]) -> Path | None:
    """返回 bases 中第一个存在 models/huggingface 目录的位置。"""
    for base in bases:
        candidate = base / "models" / "huggingface"
        if candidate.is_dir():
            return candidate
    return None


def _compiled_exe_dir() -> Path | None:
    """打包后可执行文件所在目录；开发模式返回 None。

    Nuitka onefile 中 sys.executable 指向临时解包目录里的 python.exe，
    只有 sys.argv[0] 保留原始 exe 路径。
    """
    # Nuitka 编译时不设 sys.frozen，用 __compiled__ 标记检测（官方推荐方式）
    if getattr(sys, "frozen", False) or "__compiled__" in globals():
        return Path(sys.argv[0]).resolve().parent
    return None


def _default_bases() -> list[Path]:
    """模型查找基准目录：打包后取可执行文件旁（dist 布局），开发取仓库内。"""
    if (exe_dir := _compiled_exe_dir()) is not None:
        # dist 布局：extractor/rhizome-extract.exe 或 dist 根下直接放置
        return [exe_dir, exe_dir.parent]
    return [Path(__file__).resolve().parent.parent.parent]


def _external_deps_dir() -> Path | None:
    """外置依赖目录：打包后可执行文件旁的 deps/（手动解压依赖 zip 得到）。"""
    exe_dir = _compiled_exe_dir()
    if exe_dir is None:
        return None
    candidate = exe_dir / _DEPS_DIRNAME
    return candidate if candidate.is_dir() else None


def _hf_home(bases: list[Path] | None = None) -> Path:
    """HF 缓存目录：环境变量 > 可执行文件旁 models/huggingface > ~/.rhizome。"""
    if env := os.environ.get("HF_HOME"):
        return Path(env)
    if bundled := _bundled_hf_home(bases if bases is not None else _default_bases()):
        return bundled
    return Path.home() / ".rhizome" / "models" / "huggingface"


# 外置依赖必须在导入抽取后端之前加入 sys.path；缺失时给出明确提示，
# 而不是让后续 import 抛出难以理解的 ModuleNotFoundError。
if (deps_dir := _external_deps_dir()) is not None:
    sys.path.insert(0, str(deps_dir))
elif (exe_dir := _compiled_exe_dir()) is not None:
    print(
        f"缺少外置依赖目录: {exe_dir / _DEPS_DIRNAME}\n"
        f"请下载 rhizome-extract-deps.zip 并解压到 {exe_dir}（解压后应存在 deps/ 子目录）。",
        file=sys.stderr,
    )
    raise SystemExit(EXIT_FAILED)

# 模型下载走 hf-mirror（与 desktop embedding.rs 约定一致），缓存目录按
# _hf_home 优先级确定。必须在导入 faster-whisper/huggingface_hub 之前设置。
os.environ.setdefault("HF_ENDPOINT", "https://hf-mirror.com")
os.environ.setdefault("HF_HOME", str(_hf_home()))

from rhizome_extract.extract import (  # noqa: E402
    DEFAULT_LINE_WIDTH,
    DEFAULT_MAX_LINES,
    DEFAULT_MODEL_SIZE,
    ExtractError,
    ExtractOptions,
    ExtractUnsupportedError,
    extract_text,
)
from rhizome_extract.extract.text import HanVariant  # noqa: E402

_HAN_CHOICES = {variant.value: variant for variant in HanVariant}


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="rhizome-extract",
        description="将文档/图片/音视频文件抽取为文本（Markdown / OCR / SRT）并输出到 stdout。",
    )
    parser.add_argument("file", help="待抽取的文件路径")
    parser.add_argument(
        "--han",
        choices=sorted(_HAN_CHOICES),
        default=HanVariant.AUTO.value,
        help="中文简繁转换变体（默认 auto：不转换）",
    )
    parser.add_argument("--line-width", type=int, default=DEFAULT_LINE_WIDTH, help="SRT 每行显示宽度预算")
    parser.add_argument("--max-lines", type=int, default=DEFAULT_MAX_LINES, help="SRT 每条字幕最大行数")
    parser.add_argument("--model", default=DEFAULT_MODEL_SIZE, help="faster-whisper 模型规格（默认 base）")
    parser.add_argument("--language", default=None, help="ASR 语言代码（如 zh）；缺省自动检测")
    return parser


def main(argv: list[str] | None = None) -> int:
    """Run one extraction and return the process exit code."""
    # Windows 管道默认使用本地编码（如 GBK），桌面端按 UTF-8 读取输出，
    # 这里强制统一为 UTF-8。
    for stream in (sys.stdout, sys.stderr):
        reconfigure = getattr(stream, "reconfigure", None)
        if reconfigure is not None:
            reconfigure(encoding="utf-8")

    parser = _build_parser()
    args = parser.parse_args(argv)

    source = Path(args.file)
    options = ExtractOptions(
        han=_HAN_CHOICES[args.han],
        line_width=args.line_width,
        max_lines=args.max_lines,
        model=args.model,
        language=args.language,
    )
    try:
        text = extract_text(source, options)
    except ExtractUnsupportedError as exc:
        print(f"{exc}", file=sys.stderr)
        return EXIT_UNSUPPORTED
    except ExtractError as exc:
        print(f"抽取失败: {exc}", file=sys.stderr)
        return EXIT_FAILED
    except Exception as exc:  # noqa: BLE001 - 底层库异常统一归为失败
        print(f"抽取失败: {exc}", file=sys.stderr)
        return EXIT_FAILED

    sys.stdout.write(text if text.endswith("\n") else f"{text}\n")
    return EXIT_OK


if __name__ == "__main__":
    raise SystemExit(main())
