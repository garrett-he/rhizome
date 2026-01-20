"""Docx validation.

Detection is extension-based (case-insensitive). Parsing and resource
extraction are handled by the docx backend (mammoth), so nothing happens
at this layer.
"""

from pathlib import Path

DOCX_EXTENSIONS = frozenset({".docx"})


def is_docx(path: Path) -> bool:
    """Return True if the path has a docx file extension (case-insensitive)."""
    return path.suffix.lower() in DOCX_EXTENSIONS
