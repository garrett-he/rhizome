"""Zip bomb guards shared by package-based formats (docx/pptx/xlsx).

Office documents are zip packages from untrusted sources. Before any
entry is decompressed into memory, the declared uncompressed sizes are
checked against fixed limits; exceeding them raises
:class:`ZipBombError`, which the CLI maps to a failure exit code.
"""

import zipfile
from pathlib import Path

# 单个包全部条目解压后的总量上限（1 GiB）。
MAX_TOTAL_UNCOMPRESSED = 1 << 30
# 单个条目解压后的上限（512 MiB），防止巨型嵌入对象。
MAX_ENTRY_UNCOMPRESSED = 512 << 20


class ZipBombError(ValueError):
    """Raised when a zip package exceeds safe decompression limits."""


def guard(archive: zipfile.ZipFile, source: Path) -> None:
    """Check declared uncompressed sizes before any entry is read."""
    total = 0
    for info in archive.infolist():
        if info.file_size > MAX_ENTRY_UNCOMPRESSED:
            raise ZipBombError(f"压缩包条目解压后过大: {source.name} :: {info.filename} ({info.file_size} 字节)")
        total += info.file_size
    if total > MAX_TOTAL_UNCOMPRESSED:
        raise ZipBombError(f"压缩包解压总量过大: {source.name}（{total} 字节，上限 {MAX_TOTAL_UNCOMPRESSED}）")
