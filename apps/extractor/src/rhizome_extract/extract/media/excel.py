"""Excel validation.

Detection is extension-based (case-insensitive). Parsing is handled by
the excel backend (openpyxl), so nothing happens at this layer.
"""

from pathlib import Path

EXCEL_EXTENSIONS = frozenset({".xlsx", ".xlsm"})


def is_excel(path: Path) -> bool:
    """Return True if the path has an excel file extension (case-insensitive)."""
    return path.suffix.lower() in EXCEL_EXTENSIONS
