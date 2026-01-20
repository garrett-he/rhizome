"""Pptx validation.

Detection is extension-based (case-insensitive). Parsing and resource
extraction are handled by the pptx backend (python-pptx), so nothing
happens at this layer.
"""

from pathlib import Path

PPTX_EXTENSIONS = frozenset({".pptx"})


def is_pptx(path: Path) -> bool:
    """Return True if the path has a pptx file extension (case-insensitive)."""
    return path.suffix.lower() in PPTX_EXTENSIONS
