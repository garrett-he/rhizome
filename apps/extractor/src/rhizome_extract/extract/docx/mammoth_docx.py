"""docx to Markdown conversion with mammoth.

mammoth turns the document into semantic HTML (headings, lists,
tables, links, emphasis); the rendering to Markdown lives in
:mod:`rhizome_extract.extract.formats.markdown`. Images are written to an asset
directory next to the output file instead of base64 data URIs; OLE
embedded objects cannot be located inside the document flow, so they
are extracted to the same directory and listed in an appendix section.
"""

import io
import zipfile
from collections.abc import Callable
from pathlib import Path
from typing import Any, BinaryIO, Protocol

import mammoth
from mammoth import docx as mammoth_docx

from rhizome_extract.extract.assets import assets_dir_for
from rhizome_extract.extract.formats.markdown import html_to_markdown
from rhizome_extract.extract.zipguard import guard

_CONTENT_TYPE_EXTENSIONS = {
    "image/bmp": "bmp",
    "image/gif": "gif",
    "image/jpeg": "jpg",
    "image/png": "png",
    "image/svg+xml": "svg",
    "image/tiff": "tiff",
    "image/webp": "webp",
    "image/x-emf": "emf",
    "image/x-wmf": "wmf",
}


def _ensure_wps_style_compat() -> None:
    """Teach mammoth to tolerate WPS styles lacking the w:type attribute.

    WPS writes ``<w:style>`` elements without the required ``w:type``
    attribute, which crashes mammoth's styles reader with a KeyError.
    Such styles are treated as paragraph styles (the dominant case) by
    filling the attribute before mammoth reads it.
    """
    original = mammoth_docx.read_styles_xml_element

    def tolerant_read_styles_xml_element(element: Any) -> Any:
        for child in getattr(element, "children", []):
            if getattr(child, "name", None) == "w:style":
                attributes = getattr(child, "attributes", None)
                if attributes is not None:
                    attributes.setdefault("w:type", "paragraph")
        return original(element)

    mammoth_docx.read_styles_xml_element = tolerant_read_styles_xml_element
    mammoth_docx.styles_xml.read_styles_xml_element = tolerant_read_styles_xml_element


_ensure_wps_style_compat()


class _MammothImage(Protocol):
    """The subset of the mammoth image object used by the converter."""

    content_type: str

    def open(self) -> BinaryIO:
        """Open the image data stream."""
        ...


def _extension_for(content_type: str) -> str:
    """Map an image content type to a file extension."""
    return _CONTENT_TYPE_EXTENSIONS.get(content_type.split(";")[0].strip().lower(), "bin")


def _image_handler(assets_dir: Path, counter: list[int]) -> Callable[[_MammothImage], dict[str, str]]:
    """Build a mammoth image converter that writes images into assets_dir."""

    def convert_image(image: _MammothImage) -> dict[str, str]:
        extension = _extension_for(image.content_type)
        counter[0] += 1
        name = f"image{counter[0]}.{extension}"
        assets_dir.mkdir(parents=True, exist_ok=True)
        with image.open() as data:
            (assets_dir / name).write_bytes(data.read())
        return {"src": f"{assets_dir.name}/{name}"}

    return convert_image


def _extract_embeddings(source: Path, assets_dir: Path) -> list[str]:
    """Extract OLE embedded objects from the docx package into assets_dir."""
    names: list[str] = []
    with zipfile.ZipFile(source) as archive:
        guard(archive, source)
        for info in archive.infolist():
            basename = Path(info.filename).name
            if not info.filename.startswith("word/embeddings/") or not basename:
                continue
            assets_dir.mkdir(parents=True, exist_ok=True)
            (assets_dir / basename).write_bytes(archive.read(info))
            names.append(basename)
    return names


_DRAWING_OPEN = "<w:drawing>"
_DRAWING_CLOSE = "</w:drawing>"
_TXBX_OPEN = "<w:txbxContent>"
_TXBX_CLOSE = "</w:txbxContent>"


def _find_drawing_end(xml: str, start: int) -> int:
    """Return the index past the drawing opening at start, handling nesting."""
    depth = 0
    pos = start
    while pos < len(xml):
        open_at = xml.find(_DRAWING_OPEN, pos)
        close_at = xml.find(_DRAWING_CLOSE, pos)
        if close_at == -1:
            return -1
        if open_at != -1 and open_at < close_at:
            depth += 1
            pos = open_at + len(_DRAWING_OPEN)
        else:
            depth -= 1
            pos = close_at + len(_DRAWING_CLOSE)
            if depth == 0:
                return pos
    return -1


def _unwrap_textboxes_once(xml: str) -> tuple[str, bool]:
    """Replace every textbox drawing with its txbxContent paragraphs."""
    result: list[str] = []
    replaced = False
    pos = 0
    while pos < len(xml):
        start = xml.find(_DRAWING_OPEN, pos)
        if start == -1:
            result.append(xml[pos:])
            break
        end = _find_drawing_end(xml, start)
        if end == -1:
            result.append(xml[pos:])
            break
        segment = xml[start:end]
        open_at = segment.find(_TXBX_OPEN)
        close_at = segment.rfind(_TXBX_CLOSE)
        if open_at == -1 or close_at == -1:
            result.append(xml[pos:end])
            pos = end
            continue
        inner = segment[open_at + len(_TXBX_OPEN) : close_at]
        result.append(xml[pos:start])
        if inner.lstrip().startswith("<w:p"):
            # close the surrounding run/paragraph, splice the paragraphs,
            # then reopen so the remaining XML stays well-formed
            result.append("</w:r></w:p>")
            result.append(inner)
            result.append("<w:p><w:r>")
        else:
            result.append(f"<w:p><w:r>{inner}</w:r></w:p>")
        replaced = True
        pos = end
    return "".join(result), replaced


def _unwrap_textboxes(xml: str) -> str:
    """Hoist textbox content so mammoth can see its runs and pictures.

    mammoth ignores ``wps:txbx`` textboxes entirely, losing both the
    text and any pictures anchored inside them (common in WPS-exported
    documents). Nested textboxes are unwrapped repeatedly.
    """
    for _ in range(8):
        if _TXBX_OPEN not in xml:
            return xml
        unwrapped, replaced = _unwrap_textboxes_once(xml)
        if not replaced:
            return xml
        xml = unwrapped
    return xml


def _open_for_conversion(source: Path) -> BinaryIO:
    """Open a docx package with textbox content hoisted for mammoth."""
    with zipfile.ZipFile(source) as archive:
        guard(archive, source)
        buffer = io.BytesIO()
        with zipfile.ZipFile(buffer, "w", zipfile.ZIP_DEFLATED) as copy:
            for info in archive.infolist():
                data = archive.read(info.filename)
                if info.filename == "word/document.xml":
                    data = _unwrap_textboxes(data.decode("utf-8")).encode("utf-8")
                copy.writestr(info.filename, data)
        buffer.seek(0)
        return buffer


def _append_attachments(markdown: str, attachments: list[str], assets_dir: Path) -> str:
    """Append an appendix section linking the extracted embedded objects."""
    lines = ["## 附件", ""]
    lines += [f"- [{name}]({assets_dir.name}/{name})" for name in attachments]
    return markdown.rstrip("\n") + "\n\n" + "\n".join(lines) + "\n"


def convert_docx(source: Path, output: Path) -> str:
    """Convert a docx file to Markdown.

    Images and embedded objects are extracted into ``{output.stem}_files``
    next to the output file and referenced with relative paths. Returns an
    empty string when the document has no content.
    """
    assets_dir = assets_dir_for(output)
    counter = [0]
    with _open_for_conversion(source) as stream:
        result = mammoth.convert_to_html(
            stream, convert_image=mammoth.images.img_element(_image_handler(assets_dir, counter))
        )
    markdown = html_to_markdown(result.value)
    attachments = _extract_embeddings(source, assets_dir)
    if attachments:
        markdown = _append_attachments(markdown, attachments, assets_dir)
    return markdown.strip("\n")
