"""Nuitka 编译入口：生成 rhizome-extract 单文件可执行程序。

用法（justfile build-extractor 配方调用）::

    nuitka --onefile ... apps/extractor/build_entry.py

依赖 uv sync 的 editable install 使 rhizome_extract 可导入；
rapidocr 模型随 --include-package-data=rapidocr 一并打包。
"""

from rhizome_extract.cli import main

if __name__ == "__main__":
    raise SystemExit(main())
