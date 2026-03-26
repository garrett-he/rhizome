"""收集构建产物到 dist/ 目录。

用法::

    python scripts/collect_dist.py [desktop] [any2txt]

- desktop: 复制 Tauri 裸 exe -> dist/rhizome.exe
- any2txt: 复制 any2txt 发行包 -> dist/any2txt/
           （exe + lib/site-packages + models/base）

any2txt 发行包默认取同级 any2txt 仓库的 dist/release/，可用环境变量
ANY2TXT_DIST 覆盖。产物布局与 src-tauri/src/extract.rs 的 bundled 定位一致。
"""

import argparse
import os
import shutil
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
DIST = REPO_ROOT / "dist"
DESKTOP_EXE = REPO_ROOT / "src-tauri" / "target" / "release" / "rhizome.exe"
ANY2TXT_DIST = Path(os.environ.get("ANY2TXT_DIST", REPO_ROOT.parent / "any2txt" / "dist" / "release"))


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


COLLECTORS = {
    "desktop": collect_desktop,
    "any2txt": collect_any2txt,
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
