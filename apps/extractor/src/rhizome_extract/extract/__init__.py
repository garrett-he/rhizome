"""Text extraction from multi-format documents and media.

Ported from any2txt: documents (docx/html/pptx/xlsx/pdf) become
Markdown, images become OCR text, and audio/video become SRT subtitle
text. The pipeline is media-type agnostic; each backend returns a
string and only ever writes auxiliary assets into a temporary
directory that is cleaned up on exit.
"""

import re
import tempfile
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path

from rhizome_extract.extract.asr.base import Segment  # noqa: F401 (re-export)
from rhizome_extract.extract.asr.faster_whisper import DEFAULT_MODEL_SIZE, transcribe
from rhizome_extract.extract.assets import assets_dir_for
from rhizome_extract.extract.docx import convert_docx
from rhizome_extract.extract.excel import convert_excel
from rhizome_extract.extract.formats.markdown import html_to_markdown
from rhizome_extract.extract.formats.srt import DEFAULT_LINE_WIDTH, DEFAULT_MAX_LINES, format_srt
from rhizome_extract.extract.media.audio import is_audio
from rhizome_extract.extract.media.docx import is_docx
from rhizome_extract.extract.media.excel import is_excel
from rhizome_extract.extract.media.html import copy_local_images, is_html
from rhizome_extract.extract.media.image import is_image
from rhizome_extract.extract.media.pdf import is_pdf
from rhizome_extract.extract.media.pptx import is_pptx
from rhizome_extract.extract.media.video import is_video
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


def _decode_html(source: Path) -> str:
    """Read HTML bytes with encoding fallbacks: BOM/UTF-8, declared
    charset, GB18030, and finally lossy UTF-8."""
    raw = source.read_bytes()
    try:
        return raw.decode("utf-8-sig")
    except UnicodeDecodeError:
        pass
    match = re.search(rb"""charset\s*=\s*["']?([\w.-]+)""", raw[:4096], re.IGNORECASE)
    if match:
        try:
            return raw.decode(match.group(1).decode("ascii"))
        except (UnicodeDecodeError, LookupError):
            pass
    try:
        return raw.decode("gb18030")
    except UnicodeDecodeError:
        return raw.decode("utf-8", errors="replace")


def _render_html(source: Path, output: Path) -> str:
    """Render one HTML file as Markdown, copying local images next to the output."""
    html = _decode_html(source)
    html = copy_local_images(html, source, assets_dir_for(output))
    return html_to_markdown(html).strip("\n")


def _extract_one(source: Path, output: Path, options: ExtractOptions) -> str:
    """Extract text from one supported file via the dispatch registry."""
    for claims, render in _REGISTRY:
        if claims(source):
            return render(source, output, options)
    raise ExtractUnsupportedError(f"unsupported file type: {source}")


def is_supported(path: Path) -> bool:
    """Return True if any registered media type claims the path's extension."""
    return any(claims(path) for claims, _ in _REGISTRY)


# 单一派发表：媒体类型判定与渲染器在这里配对，is_supported 与
# _extract_one 都由它派生，新增媒体类型只需追加一个条目。
_REGISTRY: list[tuple[Callable[[Path], bool], Callable[[Path, Path, ExtractOptions], str]]] = [
    (is_docx, lambda source, output, _options: convert_docx(source, output)),
    (is_pptx, lambda source, output, _options: convert_pptx(source, output)),
    (is_pdf, lambda source, output, _options: convert_pdf(source, output)),
    (is_html, lambda source, output, _options: _render_html(source, output)),
    (is_excel, lambda source, output, _options: convert_excel(source, output)),
    (is_image, lambda source, _output, options: _render_txt(source, options)),
    (is_audio, lambda source, _output, options: _render_srt(source, options)),
    (is_video, lambda source, _output, options: _render_srt(source, options)),
]


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
