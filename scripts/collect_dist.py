"""收集构建产物与模型缓存到 dist/ 目录。

用法::

    python scripts/collect_dist.py [desktop] [any2txt] [extractor] [extractor-deps] [models]

- desktop:        复制 Tauri 裸 exe -> dist/rhizome.exe
- any2txt:        复制 any2txt 发行包 -> dist/any2txt/
                  （exe + lib/site-packages + models/base）
- extractor:      复制 Nuitka onefile -> dist/extractor/rhizome-extract.exe（待删除）
- extractor-deps: 打包外置依赖 -> dist/rhizome-extract-deps.zip（待删除）
- models:         复制本机 HF 模型缓存 -> dist/models/huggingface（待删除）

any2txt 发行包默认取同级 any2txt 仓库的 dist/release/，可用环境变量
ANY2TXT_DIST 覆盖。产物布局与 desktop extract.rs 的 bundled 定位一致。
"""

import argparse
import os
import shutil
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
DIST = REPO_ROOT / "dist"
DESKTOP_EXE = REPO_ROOT / "apps" / "desktop" / "src-tauri" / "target" / "release" / "rhizome.exe"
ANY2TXT_DIST = Path(os.environ.get("ANY2TXT_DIST", REPO_ROOT.parent / "any2txt" / "dist" / "release"))
EXTRACTOR_BUILD = REPO_ROOT / "apps" / "extractor" / "build"
EXTRACTOR_DEPS = EXTRACTOR_BUILD / "deps"
EXTRACTOR_DEPS_ARCHIVE = DIST / "rhizome-extract-deps"
HF_CACHE = Path.home() / ".rhizome" / "models" / "huggingface"


def _any2txt_artifact(pattern: str) -> Path:
    matches = sorted(ANY2TXT_DIST.glob(pattern))
    if not matches:
        sys.exit(f"[dist] 缺少 any2txt 构建产物: {ANY2TXT_DIST / pattern}（先在同级 any2txt 仓库运行 just release）")
    return matches[-1]


def collect_any2txt() -> None:
    target = DIST / "any2txt"
    if target.exists():
        shutil.rmtree(target)
    target.mkdir(parents=True)
    exe = _any2txt_artifact("any2txt-*-win-x64.exe")
    shutil.copy2(exe, target / "any2txt.exe")
    shutil.unpack_archive(str(_any2txt_artifact("any2txt-*-deps-win-x64.zip")), str(target))
    shutil.unpack_archive(str(_any2txt_artifact("any2txt-*-models.zip")), str(target))
    for required in ("lib/site-packages", "models/base"):
        if not (target / required).is_dir():
            sys.exit(f"[dist] any2txt 发行包缺少 {required}")
    print(f"[dist] {target} <- {exe.parent}")


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


def collect_extractor_deps() -> None:
    if not EXTRACTOR_DEPS.is_dir():
        sys.exit(f"[dist] 缺少外置依赖目录: {EXTRACTOR_DEPS}（先跑 just build-extractor）")
    archive = shutil.make_archive(
        str(EXTRACTOR_DEPS_ARCHIVE),
        "zip",
        root_dir=EXTRACTOR_BUILD,
        base_dir=EXTRACTOR_DEPS.name,
    )
    print(f"[dist] {archive} <- {EXTRACTOR_DEPS}")
    print(f"[dist] 分发提示：将 zip 解压到 {DIST / 'extractor'}（得到 deps/ 子目录）")


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
    "any2txt": collect_any2txt,
    "extractor": collect_extractor,
    "extractor-deps": collect_extractor_deps,
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
