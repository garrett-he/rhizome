"""Input media handling, one module per media type.

Adding a new media type means adding a module here with its extension
set and a predicate, then including it in :func:`is_supported` below.
The conversion pipeline in cli.py is media-type agnostic and must stay
that way; new types are only scanned and validated here.
"""

from pathlib import Path

from rhizome_extract.extract.media.audio import is_audio
from rhizome_extract.extract.media.docx import is_docx
from rhizome_extract.extract.media.excel import is_excel
from rhizome_extract.extract.media.html import is_html
from rhizome_extract.extract.media.image import is_image
from rhizome_extract.extract.media.pdf import is_pdf
from rhizome_extract.extract.media.pptx import is_pptx
from rhizome_extract.extract.media.video import is_video

__all__ = ["is_supported"]


def is_supported(path: Path) -> bool:
    """Return True if any registered media type claims the path's extension.

    New media types plug in here.
    """
    return (
        is_video(path)
        or is_audio(path)
        or is_image(path)
        or is_docx(path)
        or is_html(path)
        or is_pptx(path)
        or is_excel(path)
        or is_pdf(path)
    )
