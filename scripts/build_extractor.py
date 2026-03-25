"""构建 rhizome-extract 单文件可执行程序与外置依赖目录。

用法::

    uv run --project apps/extractor python scripts/build_extractor.py

产物：

- ``apps/extractor/build/rhizome-extract(.exe)``：extractor 代码 + Python
  运行时与标准库。
- ``apps/extractor/build/deps/``：全部第三方依赖，由 ``collect_dist.py``
  打包成 zip，使用时需手动解压到可执行文件旁的 ``deps/``。

外置依赖目录中的 import 不在 Nuitka 的静态扫描范围内，因此必须显式
包含完整标准库，否则运行时会缺少这些库所需的标准库模块。
"""

import importlib.util
import shutil
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
EXTRACTOR_DIR = REPO_ROOT / "apps" / "extractor"
BUILD_DIR = EXTRACTOR_DIR / "build"
DEPS_DIR = BUILD_DIR / "deps"
REQUIREMENTS_FILE = BUILD_DIR / "requirements.txt"

# 不进入 exe 的第三方依赖安装目录中的非包条目
_DEPS_SKIP_NAMES = {"bin", "Scripts", "include", "share", "__pycache__"}

# 标准库中不适合（或无法）打包进 exe 的模块：平台专用、开发工具、体积过大
_STDLIB_SKIP = {
    "__future__",
    "__main__",
    "antigravity",
    "this",
    "site",
    "sitecustomize",
    "usercustomize",
    "ensurepip",
    "venv",
    "test",
    "tests",
    "idlelib",
    "tkinter",
    "turtle",
    "turtledemo",
    "pydoc_data",
    "lib2to3",
    "distutils",
    "curses",
    "_tkinter",
    "_xxsubinterpreters",
    "_xxinterpchannels",
    "_interpreters",
    "_interpchannels",
    "_interpqueues",
    "_imp",
    "_frozen_importlib",
    "_frozen_importlib_external",
}


def _run(command: list[str]) -> None:
    print(f"[build-extractor] {' '.join(command)}")
    subprocess.run(command, check=True)


def _install_deps() -> None:
    """将锁定版本的运行依赖安装到外置目录（不含项目自身）。"""
    if DEPS_DIR.exists():
        shutil.rmtree(DEPS_DIR)
    _run(
        [
            "uv",
            "export",
            "--project",
            str(EXTRACTOR_DIR),
            "--no-dev",
            "--no-hashes",
            "--no-emit-project",
            "--output-file",
            str(REQUIREMENTS_FILE),
        ]
    )
    _run(
        [
            "uv",
            "pip",
            "install",
            "--target",
            str(DEPS_DIR),
            "--python",
            sys.executable,
            "--requirements",
            str(REQUIREMENTS_FILE),
        ]
    )


def _external_import_flags() -> list[str]:
    """为外置依赖里的每个顶层包生成 --nofollow-import-to，避免打进 exe。"""
    names: set[str] = set()
    for entry in DEPS_DIR.iterdir():
        name = entry.name
        if name.startswith(".") or name in _DEPS_SKIP_NAMES:
            continue
        if ".dist-info" in name or ".data" in name:
            continue
        if entry.is_file():
            name = name.split(".", 1)[0]  # foo.py / _foo.cp313-win_amd64.pyd -> foo / _foo
        if name.isidentifier():
            names.add(name)
    return [f"--nofollow-import-to={name}" for name in sorted(names)]


def _stdlib_include_flags() -> list[str]:
    """生成完整标准库的包含参数，供外置依赖在运行时使用。"""
    flags: list[str] = []
    for name in sorted(sys.stdlib_module_names):
        if name in _STDLIB_SKIP or name.startswith("_test"):
            continue
        if name in sys.builtin_module_names:
            continue
        try:
            spec = importlib.util.find_spec(name)
        except (ImportError, ValueError):
            continue
        if spec is None:
            continue
        flag = "--include-package" if spec.submodule_search_locations else "--include-module"
        flags.append(f"{flag}={name}")
    return flags


def main() -> None:
    BUILD_DIR.mkdir(parents=True, exist_ok=True)
    _install_deps()
    _run(
        [
            sys.executable,
            "-m",
            "nuitka",
            "--onefile",
            "--assume-yes-for-downloads",
            "--remove-output",
            f"--output-dir={BUILD_DIR}",
            "--output-filename=rhizome-extract",
            "--include-package=rhizome_extract",
            *_external_import_flags(),
            *_stdlib_include_flags(),
            str(EXTRACTOR_DIR / "build_entry.py"),
        ]
    )


if __name__ == "__main__":
    main()
