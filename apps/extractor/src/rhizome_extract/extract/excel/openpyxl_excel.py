"""xlsx to Markdown conversion with openpyxl.

Each worksheet becomes a ``## {sheet name}`` section containing the
sheet's grid as a Markdown pipe table, with the first row used as the
table header. Merged cells are filled with their top-left value so the
table keeps its categorization column. Embedded pictures are extracted
into an asset directory next to the output file and referenced inside
their anchored cell. Cell line breaks become ``<br>`` and pipes are
escaped. Formula cells are rendered with their last computed value
(``data_only``). Sheets without any content are skipped.

The workbook is loaded in normal (non-read-only) mode: read-only mode
trusts the dimension metadata in the file, which WPS-generated
workbooks often write incorrectly (truncating columns), and it does not
load embedded images.
"""

import warnings
from collections import defaultdict
from pathlib import Path
from typing import Any

from openpyxl import load_workbook  # pyrefly: ignore
from openpyxl.workbook.workbook import Workbook  # pyrefly: ignore
from openpyxl.worksheet.worksheet import Worksheet  # pyrefly: ignore

from rhizome_extract.extract.assets import assets_dir_for


def _cell_to_text(value: object) -> str:
    """Render one cell value as single-line table text."""
    if value is None:
        return ""
    return str(value).replace("\r\n", "\n").replace("\n", "<br>").replace("|", "\\|").strip()


def _load_grid(worksheet: Worksheet) -> list[list[Any]]:
    """Load the sheet values as a dense grid, filling merged cells.

    Merged ranges repeat their top-left value so category columns keep
    meaning on every row. The grid is padded to a rectangle.
    """
    grid = [list(row) for row in worksheet.iter_rows(values_only=True)]
    if not grid:
        return []
    width = max(len(row) for row in grid)
    grid = [row + [None] * (width - len(row)) for row in grid]
    for rng in worksheet.merged_cells.ranges:
        if rng.min_row > len(grid) or rng.min_col > width:
            continue
        top = grid[rng.min_row - 1][rng.min_col - 1]
        for row in range(rng.min_row, min(rng.max_row, len(grid)) + 1):
            for col in range(rng.min_col, min(rng.max_col, width) + 1):
                grid[row - 1][col - 1] = top
    return grid


def _collect_images(worksheet: Worksheet, assets_dir: Path, counter: list[int]) -> dict[tuple[int, int], list[str]]:
    """Extract embedded pictures and map their anchors to cell positions."""
    refs: dict[tuple[int, int], list[str]] = defaultdict(list)
    for image in getattr(worksheet, "_images", []):
        origin = getattr(getattr(image, "anchor", None), "_from", None)
        if origin is None:
            continue
        counter[0] += 1
        name = f"image{counter[0]}.{image.format}"
        assets_dir.mkdir(parents=True, exist_ok=True)
        (assets_dir / name).write_bytes(image._data())
        refs[(origin.row, origin.col)].append(name)
    return refs


def _cell_content(value: object, image_names: list[str], assets_dir: Path) -> str:
    """Render one cell: its text plus any anchored picture references."""
    parts = [text for text in [_cell_to_text(value)] if text]
    parts += [f"![{name}]({assets_dir.name}/{name})" for name in image_names]
    return "<br>".join(parts)


def _sheet_to_lines(worksheet: Worksheet, refs: dict[tuple[int, int], list[str]], assets_dir: Path) -> list[str]:
    """Render one worksheet grid as Markdown pipe-table lines."""
    grid = _load_grid(worksheet)
    if not grid:
        return []
    width = max(len(row) for row in grid)
    for row, col in refs:
        while len(grid) <= row:
            grid.append([None] * width)
        if col >= width:
            for existing in grid:
                existing.extend([None] * (col + 1 - width))
            width = col + 1
    grid = [
        [_cell_content(value, refs.get((row, col), []), assets_dir) for col, value in enumerate(grid_row)]
        for row, grid_row in enumerate(grid)
    ]
    while grid and all(cell == "" for cell in grid[-1]):
        grid.pop()
    if not grid:
        return []
    lines = [
        "| " + " | ".join(grid[0]) + " |",
        "| " + " | ".join(["---"] * width) + " |",
    ]
    lines += ["| " + " | ".join(row) + " |" for row in grid[1:]]
    return lines


def _sections(workbook: Workbook, assets_dir: Path, counter: list[int]) -> list[str]:
    """Render every non-empty worksheet as a ``## `` section."""
    sections: list[str] = []
    for worksheet in workbook.worksheets:
        refs = _collect_images(worksheet, assets_dir, counter)
        lines = _sheet_to_lines(worksheet, refs, assets_dir)
        if not lines:
            continue
        sections.append("\n".join([f"## {worksheet.title}", "", *lines]))
    return sections


def convert_excel(source: Path, output: Path) -> str:
    """Convert an excel file to Markdown, one ``## `` section per sheet.

    Embedded pictures are extracted into ``{output.stem}_files`` next to
    the output file and referenced inside their anchored cells. Returns
    an empty string when the workbook has no content at all.
    """
    assets_dir = assets_dir_for(output)
    counter = [0]
    with warnings.catch_warnings():
        # WPS-generated workbooks miss the default style; that is harmless.
        warnings.simplefilter("ignore", UserWarning)
        workbook = load_workbook(source, data_only=True)
        try:
            return "\n\n".join(_sections(workbook, assets_dir, counter)).strip("\n")
        finally:
            workbook.close()
