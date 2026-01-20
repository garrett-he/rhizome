"""Image validation.

Detection is extension-based (case-insensitive). Decoding and text
recognition are handled by the OCR backend (RapidOCR), so no external
dependency is required at this layer.
"""

from pathlib import Path

IMAGE_EXTENSIONS = frozenset(
    {
        ".bmp",
        ".jpeg",
        ".jpg",
        ".png",
        ".tiff",
        ".webp",
    }
)


def is_image(path: Path) -> bool:
    """Return True if the path has an image file extension (case-insensitive)."""
    return path.suffix.lower() in IMAGE_EXTENSIONS
