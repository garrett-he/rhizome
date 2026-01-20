"""pdf to Markdown conversion with PyMuPDF.

PDF has no built-in heading structure, so the body font size is
inferred from the document-wide size distribution and text blocks whose
dominant size is significantly larger become ATX headings. Tables are
detected by PyMuPDF's table finder and rendered as Markdown pipe
tables; pictures are extracted into an asset directory next to the
output file and referenced with relative paths; URI links become
Markdown links. Following docx conversion rules, no script conversion
is applied.
"""

from collections.abc import Callable
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, cast

import pymupdf as fitz

from rhizome_extract.extract.assets import assets_dir_for

_HEADING_RATIOS = ((1.6, "#"), (1.3, "##"), (1.15, "###"))
_MAX_HEADING_LENGTH = 120

# PyMuPDF text-extraction dictionaries (block/line/span) have string keys.
type SpanDict = dict[str, Any]
type BlockDict = dict[str, Any]
type LinkDict = dict[str, Any]


@dataclass
class _Image:
    """One placement of a page image, sorted into the reading order.

    Only the xref is stored during collection; image bytes are
    extracted lazily and deduplicated by xref at render time.
    """

    y: float
    x: float
    xref: int


@dataclass
class _Table:
    """One detected table with its position and extracted rows."""

    y: float
    x: float
    rows: list[list[str]]


@dataclass
class _PageData:
    """Everything needed to render one page."""

    blocks: list[BlockDict] = field(default_factory=list)
    tables: list[_Table] = field(default_factory=list)
    images: list[_Image] = field(default_factory=list)
    links: list[LinkDict] = field(default_factory=list)


def _span_is_bold(span: SpanDict) -> bool:
    """Return True if a text span renders in a bold face."""
    return bool(span["flags"] & 16) or "bold" in span["font"].lower()


def _span_is_italic(span: SpanDict) -> bool:
    """Return True if a text span renders in an italic face."""
    italic = "italic" in span["font"].lower() or "oblique" in span["font"].lower()
    return bool(span["flags"] & 2) or italic


def _body_size(pages: list[_PageData]) -> float:
    """Return the dominant text size across the document, weighted by length."""
    sizes: list[float] = []
    for page in pages:
        for block in page.blocks:
            if block["type"] != 0:
                continue
            for line in block["lines"]:
                for span in line["spans"]:
                    sizes += [span["size"]] * len(span["text"].strip())
    if not sizes:
        return 0.0
    return max(set(sizes), key=sizes.count)


def _heading_prefix(block: BlockDict, body: float) -> str:
    """Return the ATX heading prefix for a block, or an empty string."""
    if body <= 0:
        return ""
    spans = [span for line in block["lines"] for span in line["spans"]]
    largest = max((span["size"] for span in spans), default=0.0)
    text_length = sum(len(span["text"].strip()) for span in spans)
    if text_length > _MAX_HEADING_LENGTH:
        return ""
    for ratio, prefix in _HEADING_RATIOS:
        if largest >= body * ratio:
            return f"{prefix} "
    return ""


def _needs_space(left: str, right: str) -> bool:
    """Return True if two joined lines need a space at the boundary."""
    return any(char.isascii() and char.isalnum() for char in (left, right))


def _join_lines(lines: list[str]) -> str:
    """Join the lines of one block, spacing only ASCII word boundaries."""
    joined = ""
    for line in lines:
        if not joined:
            joined = line
        elif _needs_space(joined[-1], line[0]):
            joined += f" {line}"
        else:
            joined += line
    return joined


def _render_line(line: SpanDict, links: list[LinkDict], heading: bool) -> str:
    """Render one line of spans with emphasis and URI links."""
    rendered = ""
    for span in line["spans"]:
        text = span["text"]
        if not heading:
            if _span_is_bold(span):
                text = f"**{text}**"
            elif _span_is_italic(span):
                text = f"*{text}*"
        for link in links:
            if link["kind"] == fitz.LINK_URI and fitz.Rect(span["bbox"]).intersects(link["from"]):
                text = f"[{text}]({link['uri']})"
                break
        rendered += text
    return rendered


def _render_block(block: BlockDict, links: list[LinkDict], body: float) -> str | None:
    """Render one text block as a heading or a paragraph."""
    lines = [_render_line(line, links, heading=False) for line in block["lines"]]
    lines = [line for line in lines if line.strip()]
    if not lines:
        return None
    prefix = _heading_prefix(block, body)
    if prefix:
        heading_lines = [_render_line(line, links, heading=True) for line in block["lines"]]
        return f"{prefix}{_join_lines([line for line in heading_lines if line.strip()])}"
    return _join_lines(lines)


def _render_table(table: _Table) -> list[str]:
    """Render a detected table as Markdown pipe-table lines."""
    if not table.rows:
        return []
    width = max(len(row) for row in table.rows)
    padded = [row + [""] * (width - len(row)) for row in table.rows]
    lines = [
        "| " + " | ".join(padded[0]) + " |",
        "| " + " | ".join(["---"] * width) + " |",
    ]
    lines += ["| " + " | ".join(row) + " |" for row in padded[1:]]
    return lines


def _clean_cell(value: str | None) -> str:
    """Render one table cell as single-line table text."""
    if value is None:
        return ""
    return value.replace("\n", " ").replace("|", "\\|").strip()


def _inside_any(x: float, y: float, tables: list[_Table]) -> bool:
    """Return True if an x/y point falls inside any table's bbox."""
    return any(table.x <= x and y <= table.y for table in tables)


def _collect_page(page: fitz.Page) -> _PageData:
    """Collect the renderable content of one page.

    Image bytes are not extracted here; only xrefs and placements are
    recorded so repeated images are deduplicated at render time.
    """
    text_dict = cast("dict[str, Any]", page.get_text("dict"))
    finder = page.find_tables()
    tables = finder.tables if finder is not None else []
    data = _PageData(blocks=text_dict["blocks"], links=page.get_links())
    for table in tables:
        rows = [[_clean_cell(cell) for cell in row] for row in table.extract()]
        bbox = table.bbox
        data.tables.append(_Table(y=bbox[3], x=bbox[0], rows=rows))
    for info in page.get_images(full=True):
        for rect in page.get_image_rects(info[0]):
            data.images.append(_Image(y=rect.y1, x=rect.x0, xref=info[0]))
    return data


def _render_page(data: _PageData, number: int, resolve_asset: Callable[[int], str], body: float) -> list[str]:
    """Render one page's content in reading order."""
    elements: list[tuple[float, float, str]] = []
    for block in data.blocks:
        if block["type"] != 0:
            continue
        center_x = (block["bbox"][0] + block["bbox"][2]) / 2
        center_y = (block["bbox"][1] + block["bbox"][3]) / 2
        if _inside_any(center_x, center_y, data.tables):
            continue
        rendered = _render_block(block, data.links, body)
        if rendered:
            elements.append((block["bbox"][1], block["bbox"][0], rendered))
    for table in data.tables:
        lines = _render_table(table)
        if lines:
            elements.append((table.y, table.x, "\n".join(lines)))
    for image in data.images:
        ref = resolve_asset(image.xref)
        elements.append((image.y, image.x, f"![{Path(ref).name}]({ref})"))
    elements.sort(key=lambda element: (element[0], element[1]))
    return [element[2] for element in elements]


def convert_pdf(source: Path, output: Path) -> str:
    """Convert a pdf file to Markdown.

    Pictures are extracted into ``{output.stem}_files`` next to the
    output file and referenced with relative paths. The same embedded
    image (xref) is extracted and written once regardless of how many
    pages or placements reference it. Returns an empty string when the
    document has no content.
    """
    assets_dir = assets_dir_for(output)
    sections: list[str] = []
    with fitz.open(source) as doc:
        pages = [_collect_page(page) for page in doc]
        body = _body_size(pages)
        written: dict[int, str] = {}

        def resolve_asset(xref: int) -> str:
            """Extract image bytes for xref once and return its relative ref."""
            ref = written.get(xref)
            if ref is None:
                extracted = doc.extract_image(xref)
                assets_dir.mkdir(parents=True, exist_ok=True)
                ref = f"{assets_dir.name}/image{len(written) + 1}.{extracted['ext']}"
                (output.parent / ref).write_bytes(extracted["image"])
                written[xref] = ref
            return ref

        for number, data in enumerate(pages, start=1):
            rendered = _render_page(data, number, resolve_asset, body)
            if rendered:
                sections.append("\n\n".join(rendered))
    return "\n\n".join(sections).strip("\n")
