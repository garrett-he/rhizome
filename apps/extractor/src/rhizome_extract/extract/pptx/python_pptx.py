"""pptx to Markdown conversion with python-pptx.

Each slide becomes a ``## `` section headed by its title (or
``## Slide N``). Body text is rendered as bullet lists using the
paragraph indent level; tables become Markdown pipe tables; pictures
are extracted into an asset directory next to the output file and
referenced with relative paths; speaker notes are kept as blockquotes.
"""

import zipfile
from pathlib import Path

from pptx import Presentation
from pptx.oxml.ns import qn
from pptx.shapes.autoshape import Shape
from pptx.shapes.base import BaseShape
from pptx.shapes.graphfrm import GraphicFrame
from pptx.shapes.group import GroupShape
from pptx.shapes.picture import Picture
from pptx.slide import Slide
from pptx.text.text import TextFrame, _Paragraph

from rhizome_extract.extract.assets import assets_dir_for
from rhizome_extract.extract.zipguard import guard


def _slide_title(slide: Slide) -> str:
    """Return the slide title text, or an empty string when absent."""
    shape = slide.shapes.title
    if isinstance(shape, Shape):
        return shape.text_frame.text.strip()
    return ""


def _is_bulleted(paragraph: _Paragraph) -> bool:
    """Return True if the paragraph carries explicit bullet formatting."""
    p_pr = paragraph._p.find(qn("a:pPr"))
    if p_pr is None:
        return False
    return p_pr.find(qn("a:buChar")) is not None or p_pr.find(qn("a:buAutoNum")) is not None


def _render_paragraphs(text_frame: TextFrame, bulleted_default: bool) -> list[str]:
    """Render a text frame's paragraphs, indenting by paragraph level."""
    lines: list[str] = []
    for paragraph in text_frame.paragraphs:
        text = paragraph.text.strip()
        if not text:
            continue
        indent = "  " * paragraph.level
        if bulleted_default or _is_bulleted(paragraph):
            lines.append(f"{indent}- {text}")
        else:
            lines.append(f"{indent}{text}")
    return lines


def _render_table(shape: GraphicFrame) -> list[str]:
    """Render a pptx table as a Markdown pipe table."""
    rows = [
        [cell.text.strip().replace("\n", " ").replace("|", "\\|") for cell in row.cells] for row in shape.table.rows
    ]
    rows = [row for row in rows if row]
    if not rows:
        return []
    width = max(len(row) for row in rows)
    padded = [row + [""] * (width - len(row)) for row in rows]
    lines = [
        "| " + " | ".join(padded[0]) + " |",
        "| " + " | ".join(["---"] * width) + " |",
    ]
    lines += ["| " + " | ".join(row) + " |" for row in padded[1:]]
    return lines


def _render_picture(shape: Picture, number: int, assets_dir: Path, counter: list[int]) -> str | None:
    """Extract a slide picture into assets_dir and reference it.

    Returns None for pictures python-pptx cannot decode: linked-only
    pictures ("link to file", ValueError) and images stored in a format
    python-pptx does not recognize, e.g. ``image/unknown`` (AttributeError).
    """
    try:
        image = shape.image
    except (ValueError, AttributeError):
        return None
    counter[0] += 1
    name = f"slide{number}_image{counter[0]}.{image.ext}"
    assets_dir.mkdir(parents=True, exist_ok=True)
    (assets_dir / name).write_bytes(image.blob)
    return f"![{name}]({assets_dir.name}/{name})"


def _render_shape(
    shape: BaseShape,
    number: int,
    assets_dir: Path,
    counter: list[int],
    title_shape: BaseShape | None,
) -> list[str]:
    """Render one shape, recursing into groups."""
    if isinstance(shape, GroupShape):
        lines: list[str] = []
        for child in shape.shapes:
            lines += _render_shape(child, number, assets_dir, counter, None)
        return lines
    if isinstance(shape, GraphicFrame) and shape.has_table:
        return _render_table(shape)
    if isinstance(shape, Picture):
        picture = _render_picture(shape, number, assets_dir, counter)
        return [picture] if picture is not None else []
    if isinstance(shape, Shape) and shape is not title_shape:
        # body placeholders are bulleted by default in PowerPoint layouts
        return _render_paragraphs(shape.text_frame, shape.is_placeholder)
    return []


def _render_notes(slide: Slide) -> list[str]:
    """Render speaker notes as blockquotes."""
    if not slide.has_notes_slide:
        return []
    notes_frame = slide.notes_slide.notes_text_frame
    if notes_frame is None:
        return []
    text = notes_frame.text.strip()
    return [f"> {line}" for line in text.splitlines() if line.strip()]


def _render_slide(slide: Slide, number: int, assets_dir: Path, counter: list[int]) -> str:
    """Render one slide as a Markdown section."""
    title_shape = slide.shapes.title
    title = _slide_title(slide)
    lines = [f"## {title}" if title else f"## Slide {number}"]
    for shape in slide.shapes:
        shape_lines = _render_shape(shape, number, assets_dir, counter, title_shape)
        if shape_lines:
            lines.append("")
            lines += shape_lines
    notes = _render_notes(slide)
    if notes:
        lines.append("")
        lines += notes
    return "\n".join(lines)


def convert_pptx(source: Path, output: Path) -> str:
    """Convert a pptx file to Markdown.

    Slide pictures are extracted into ``{output.stem}_files`` next to the
    output file and referenced with relative paths. Returns an empty
    string when the presentation has no content.
    """
    assets_dir = assets_dir_for(output)
    counter = [0]
    with zipfile.ZipFile(source) as archive:
        guard(archive, source)
    presentation = Presentation(str(source))
    sections = [
        _render_slide(slide, number, assets_dir, counter) for number, slide in enumerate(presentation.slides, start=1)
    ]
    return "\n\n".join(sections).strip("\n")
