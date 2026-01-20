"""HTML to Markdown rendering built on markdownify.

Pure text-in/text-out rendering shared by the docx and html conversion
paths. ATX headings and dash bullets are enforced for stable output,
and the first table row is inferred as the header so mammoth's
header-less docx tables still produce valid Markdown tables.
"""

from bs4 import Tag
from markdownify import MarkdownConverter


class PipeSafeMarkdownConverter(MarkdownConverter):
    """MarkdownConverter that escapes pipes inside table cells.

    markdownify's native table rendering leaves ``|`` characters in cell
    text untouched, which breaks the surrounding pipe table.
    """

    def _escape_cell(self, el: Tag, text: str, parent_tags: tuple[str, ...] | None) -> str:
        """Render one cell with the native converter, escaping its text."""
        return super().convert_td(el, text.replace("|", "\\|"), parent_tags)  # type: ignore[arg-type]

    def convert_td(self, el: Tag, text: str, parent_tags: tuple[str, ...] | None = None) -> str:
        """Convert a table data cell with pipe escaping."""
        return self._escape_cell(el, text, parent_tags)

    def convert_th(self, el: Tag, text: str, parent_tags: tuple[str, ...] | None = None) -> str:
        """Convert a table header cell with pipe escaping."""
        return self._escape_cell(el, text, parent_tags)


def html_to_markdown(html: str) -> str:
    """Convert an HTML fragment to Markdown."""
    converter = PipeSafeMarkdownConverter(
        heading_style="ATX",
        bullets="-",
        table_infer_header=True,
    )
    return converter.convert(html)
