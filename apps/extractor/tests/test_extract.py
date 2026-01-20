"""Golden and dispatch tests for the extraction pipeline."""

from pathlib import Path

import pytest

from conftest import make_docx, make_html, make_pdf, make_pptx, make_xlsx  # pyrefly: ignore[missing-import]
from rhizome_extract.extract import ExtractOptions, ExtractUnsupportedError, extract_text
from rhizome_extract.extract.asr.base import Segment
from rhizome_extract.extract.formats.srt import format_srt
from rhizome_extract.extract.media import is_supported
from rhizome_extract.extract.text import HanVariant, convert_han


def test_extract_docx_yields_paragraphs(tmp_path: Path) -> None:
    doc = make_docx(tmp_path / "a.docx")

    text = extract_text(doc)

    assert "服务拆分要注意业务边界" in text
    assert "第二个段落。" in text


def test_extract_xlsx_yields_markdown_table(tmp_path: Path) -> None:
    sheet = make_xlsx(tmp_path / "a.xlsx")

    text = extract_text(sheet)

    assert "## 服务清单" in text
    assert "服务" in text
    assert "网关" in text


def test_extract_pptx_yields_slide_text(tmp_path: Path) -> None:
    deck = make_pptx(tmp_path / "a.pptx")

    text = extract_text(deck)

    assert "架构评审要点" in text


def test_extract_pdf_yields_text(tmp_path: Path) -> None:
    document = make_pdf(tmp_path / "a.pdf")

    text = extract_text(document)

    assert "PDF extraction check" in text


def test_extract_html_yields_markdown(tmp_path: Path) -> None:
    page = make_html(tmp_path / "a.html")

    text = extract_text(page)

    assert "# 导入指南" in text
    assert "先选择知识库" in text


def test_extract_unsupported_type(tmp_path: Path) -> None:
    bogus = tmp_path / "a.xyz"
    bogus.write_bytes(b"???")

    with pytest.raises(ExtractUnsupportedError):
        extract_text(bogus)


def test_extract_image_dispatches_to_ocr(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    image = tmp_path / "a.png"
    image.write_bytes(b"\x89PNG\r\n\x1a\n" + b"rest")
    monkeypatch.setattr("rhizome_extract.extract.recognize", lambda _path: "OCR 识别结果")

    text = extract_text(image)

    assert text == "OCR 识别结果"


def test_extract_audio_dispatches_to_asr_and_renders_srt(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    audio = tmp_path / "a.mp3"
    audio.write_bytes(b"ID3 fake audio")

    def fake_transcribe(_path: Path, *, model_size: str, language: str | None):
        assert model_size == "base"
        assert language is None
        return iter([Segment(start=0.0, end=1.5, text="第一句话"), Segment(start=1.5, end=3.0, text="第二句话")])

    monkeypatch.setattr("rhizome_extract.extract.transcribe", fake_transcribe)

    text = extract_text(audio)

    assert "1\n00:00:00,000 --> " in text
    assert "第一句话" in text
    assert "第二句话" in text


def test_han_conversion_round_trip() -> None:
    source = "汉字转换测试"

    traditional = convert_han(source, HanVariant.TRADITIONAL)
    assert traditional != source
    assert convert_han(traditional, HanVariant.SIMPLIFIED) == source
    # auto 不转换
    assert convert_han(source, HanVariant.AUTO) == source


def test_format_srt_skips_empty_segments_and_splits_long_cues() -> None:
    segments = [
        Segment(start=0.0, end=1.0, text=""),
        Segment(start=1.0, end=2.0, text="短句"),
        Segment(start=2.0, end=12.0, text="很长" * 60),
    ]

    rendered = format_srt(segments)

    cues = rendered.split("\n\n")
    # 空段跳过；短句 1 条；长句（120 个全角字符 ≈ 240 宽 / 42 ≈ 6 行）拆 3 条
    assert len(cues) == 4
    assert cues[0].startswith("1\n")
    assert "短句" in cues[0]
    # 长句按 max_lines 拆分：占多条 cue
    assert "很长" in cues[1]
    assert "很长" in cues[2]
    assert "很长" in cues[3]


def test_extract_options_defaults_match_srt_conventions() -> None:
    options = ExtractOptions()

    assert options.line_width == 42
    assert options.max_lines == 2
    assert options.han == HanVariant.AUTO


def test_is_supported_covers_all_media_types(tmp_path: Path) -> None:
    extensions = [
        "a.docx",
        "b.xlsx",
        "c.pptx",
        "d.pdf",
        "e.html",
        "f.htm",
        "g.png",
        "h.jpg",
        "i.mp3",
        "j.wav",
        "k.mp4",
        "l.mkv",
    ]
    for name in extensions:
        assert is_supported(tmp_path / name), name
    # 大小写不敏感
    assert is_supported(tmp_path / "A.DOCX")
    assert not is_supported(tmp_path / "a.xyz")
