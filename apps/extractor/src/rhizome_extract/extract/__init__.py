"""Text extraction from multi-format documents and media.

Ported from any2txt: documents (docx/html/pptx/xlsx/pdf) become
Markdown, images become OCR text, and audio/video become SRT subtitle
text. The pipeline is media-type agnostic; each backend returns a
string and only ever writes auxiliary assets into a temporary
directory that is cleaned up on exit.
"""

import tempfile
from dataclasses import dataclass
from pathlib import Path

from rhizome_extract.extract.asr.base import Segment  # noqa: F401 (re-export)
from rhizome_extract.extract.asr.faster_whisper import DEFAULT_MODEL_SIZE, transcribe
from rhizome_extract.extract.assets import assets_dir_for
from rhizome_extract.extract.docx import convert_docx
from rhizome_extract.extract.excel import convert_excel
from rhizome_extract.extract.formats.markdown import html_to_markdown
from rhizome_extract.extract.formats.srt import DEFAULT_LINE_WIDTH, DEFAULT_MAX_LINES, format_srt
from rhizome_extract.extract.media import is_supported
from rhizome_extract.extract.media.docx import is_docx
from rhizome_extract.extract.media.excel import is_excel
from rhizome_extract.extract.media.html import copy_local_images, is_html
from rhizome_extract.extract.media.image import is_image
from rhizome_extract.extract.media.pdf import is_pdf
from rhizome_extract.extract.media.pptx import is_pptx
from rhizome_extract.extract.ocr import recognize
from rhizome_extract.extract.pdf import convert_pdf
from rhizome_extract.extract.pptx import convert_pptx
from rhizome_extract.extract.text import HanVariant, convert_han

__all__ = [
    "DEFAULT_LINE_WIDTH",
    "DEFAULT_MAX_LINES",
    "DEFAULT_MODEL_SIZE",
    "ExtractError",
    "ExtractOptions",
    "ExtractUnsupportedError",
    "HanVariant",
    "extract_text",
    "is_supported",
]


class ExtractError(RuntimeError):
    """Raised when extraction of a supported file fails."""


class ExtractUnsupportedError(ExtractError):
    """Raised when no registered media type claims the file."""


@dataclass(frozen=True)
class ExtractOptions:
    """Extraction options; defaults mirror the any2txt CLI defaults."""

    han: HanVariant = HanVariant.AUTO
    line_width: int = DEFAULT_LINE_WIDTH
    max_lines: int = DEFAULT_MAX_LINES
    model: str = DEFAULT_MODEL_SIZE
    language: str | None = None


def _render_srt(source: Path, options: ExtractOptions) -> str:
    """Transcribe one media file and return its rendered SRT text."""
    text = format_srt(
        transcribe(source, model_size=options.model, language=options.language),
        line_width=options.line_width,
        max_lines=options.max_lines,
    )
    return convert_han(text, options.han)


def _render_txt(source: Path, options: ExtractOptions) -> str:
    """Recognize one image file and return its plain text."""
    return convert_han(recognize(source), options.han)


def _render_html(source: Path, output: Path) -> str:
    """Render one HTML file as Markdown, copying local images next to the output."""
    html = source.read_text(encoding="utf-8-sig")
    html = copy_local_images(html, source, assets_dir_for(output))
    return html_to_markdown(html).strip("\n")


def _extract_one(source: Path, output: Path, options: ExtractOptions) -> str:
    """Extract text from one supported file, dispatching by media type."""
    if is_docx(source):
        return convert_docx(source, output)
    if is_pptx(source):
        return convert_pptx(source, output)
    if is_pdf(source):
        return convert_pdf(source, output)
    if is_html(source):
        return _render_html(source, output)
    if is_excel(source):
        return convert_excel(source, output)
    if is_image(source):
        return _render_txt(source, options)
    return _render_srt(source, options)


def extract_text(path: Path, options: ExtractOptions | None = None) -> str:
    """Extract text from ``path`` and return it as a string.

    Documents yield Markdown, images yield OCR text, and audio/video
    yield SRT subtitle text. Auxiliary assets are written beside a
    scratch output file inside a temporary directory that is removed on
    return. Raises :class:`ExtractUnsupportedError` for files no media
    type claims and :class:`ExtractError` (or the underlying exception)
    when extraction fails.
    """
    if not is_supported(path):
        raise ExtractUnsupportedError(f"unsupported file type: {path}")
    opts = options if options is not None else ExtractOptions()
    with tempfile.TemporaryDirectory(prefix="rhizome-extract-") as tmp:
        output = Path(tmp) / "extracted.md"
        return _extract_one(path, output, opts)
