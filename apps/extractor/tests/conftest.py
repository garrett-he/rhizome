"""Sample file builders shared by the extractor tests."""

import zipfile
from pathlib import Path

from openpyxl import Workbook
from pptx import Presentation
from pptx.util import Pt

_MINIMAL_DOCX_XML = (
    '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
    '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">'
    '<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>'
    '<Default Extension="xml" ContentType="application/xml"/>'
    '<Override PartName="/word/document.xml" '
    'ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>'
    "</Types>"
)
_MINIMAL_RELS_XML = (
    '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
    '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
    '<Relationship Id="rId1" '
    'Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" '
    'Target="word/document.xml"/>'
    "</Relationships>"
)
_MINIMAL_DOCUMENT_XML = (
    '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
    '<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">'
    "<w:body>"
    "<w:p><w:r><w:t>服务拆分要注意业务边界</w:t></w:r></w:p>"
    "<w:p><w:r><w:t>第二个段落。</w:t></w:r></w:p>"
    "</w:body>"
    "</w:document>"
)


def make_docx(path: Path) -> Path:
    """Write a minimal (but valid) docx with two paragraphs."""
    with zipfile.ZipFile(path, "w") as archive:
        archive.writestr("[Content_Types].xml", _MINIMAL_DOCX_XML)
        archive.writestr("_rels/.rels", _MINIMAL_RELS_XML)
        archive.writestr("word/document.xml", _MINIMAL_DOCUMENT_XML)
    return path


def make_xlsx(path: Path) -> Path:
    """Write a one-sheet xlsx with a header row and one data row."""
    workbook = Workbook()
    sheet = workbook.active
    sheet.title = "服务清单"
    sheet.append(["服务", "负责人"])
    sheet.append(["网关", "Garrett"])
    workbook.save(path)
    return path


def make_pptx(path: Path) -> Path:
    """Write a one-slide pptx with a title textbox."""
    presentation = Presentation()
    slide = presentation.slides.add_slide(presentation.slide_layouts[5])
    box = slide.shapes.add_textbox(Pt(40), Pt(40), Pt(400), Pt(80))
    box.text_frame.text = "架构评审要点"
    presentation.save(path)
    return path


def make_pdf(path: Path) -> Path:
    """Write a one-page pdf with a text line."""
    import pymupdf

    document = pymupdf.open()
    page = document.new_page()
    # 默认 helv 字体不支持 CJK，fixture 使用 ASCII 文本
    page.insert_text((72, 72), "PDF extraction check", fontsize=12)
    document.save(path)
    document.close()
    return path


def make_html(path: Path) -> Path:
    """Write a simple HTML file with a heading and a paragraph."""
    path.write_text(
        "<html><body><h1>导入指南</h1><p>先选择知识库，再导入文档。</p></body></html>",
        encoding="utf-8",
    )
    return path
