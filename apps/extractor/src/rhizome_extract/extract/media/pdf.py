"""Pdf validation.

Detection is extension-based (case-insensitive). Parsing and resource
extraction are handled by the pdf backend (PyMuPDF), so nothing happens
at this layer.
"""

from pathlib import Path

PDF_EXTENSIONS = frozenset({".pdf"})


def is_pdf(path: Path) -> bool:
    """Return True if the path has a pdf file extension (case-insensitive)."""
    return path.suffix.lower() in PDF_EXTENSIONS
