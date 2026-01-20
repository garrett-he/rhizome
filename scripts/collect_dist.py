"""收集构建产物与模型缓存到 dist/ 目录。

用法::

    python scripts/collect_dist.py [desktop] [extractor] [models]

- desktop:   复制 Tauri 裸 exe -> dist/rhizome.exe
- extractor: 复制 Nuitka onefile -> dist/extractor/rhizome-extract.exe
- models:    复制本机 HF 模型缓存 -> dist/models/huggingface（缺失时跳过）

产物布局与 desktop extract.rs 的 bundled 定位、CLI _hf_home 的
bundled 查找逻辑一一对应。
"""

import argparse
import shutil
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
DIST = REPO_ROOT / "dist"
DESKTOP_EXE = REPO_ROOT / "apps" / "desktop" / "src-tauri" / "target" / "release" / "rhizome.exe"
EXTRACTOR_BUILD = REPO_ROOT / "apps" / "extractor" / "build"
HF_CACHE = Path.home() / ".rhizome" / "models" / "huggingface"


def collect_desktop() -> None:
    source = DESKTOP_EXE
    if not source.is_file():
        source = source.with_suffix("")  # 非 Windows 无 .exe 后缀
    if not source.is_file():
        sys.exit(f"[dist] 缺少 desktop 构建产物: {DESKTOP_EXE}（先跑 just build-desktop）")
    shutil.copy2(source, DIST / source.name)
    print(f"[dist] {DIST / source.name} <- {source}")


def collect_extractor() -> None:
    source = EXTRACTOR_BUILD / "rhizome-extract.exe"
    if not source.is_file():
        source = source.with_suffix("")  # 非 Windows 无 .exe 后缀
    if not source.is_file():
        sys.exit(f"[dist] 缺少 extractor 构建产物: {EXTRACTOR_BUILD}（先跑 just build-extractor）")
    target = DIST / "extractor" / source.name
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, target)
    print(f"[dist] {target} <- {source}")


def collect_models() -> None:
    if not HF_CACHE.is_dir():
        print(f"[dist] 未找到 HF 模型缓存 {HF_CACHE}，跳过模型复制（首次 ASR 时将自动下载）")
        return
    target = DIST / "models" / "huggingface"
    if target.exists():
        shutil.rmtree(target)
    shutil.copytree(HF_CACHE, target)
    print(f"[dist] {target} <- {HF_CACHE}")


COLLECTORS = {
    "desktop": collect_desktop,
    "extractor": collect_extractor,
    "models": collect_models,
}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "targets",
        nargs="*",
        default=list(COLLECTORS),
        choices=list(COLLECTORS),
        help="要收集的产物（缺省全部）",
    )
    args = parser.parse_args()
    DIST.mkdir(parents=True, exist_ok=True)
    for name in args.targets:
        COLLECTORS[name]()


if __name__ == "__main__":
    main()
