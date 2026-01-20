"""End-to-end tests for the rhizome-extract CLI."""

import os
import subprocess
import sys
from pathlib import Path

from conftest import make_html, make_pdf  # pyrefly: ignore[missing-import]
from rhizome_extract.cli import _bundled_hf_home, _default_bases, _hf_home, main


def test_hf_home_prefers_environment_variable(
    tmp_path: Path, monkeypatch
) -> None:  # noqa: ANN001
    monkeypatch.setenv("HF_HOME", str(tmp_path / "env-cache"))

    assert _hf_home(bases=[]) == tmp_path / "env-cache"


def test_hf_home_falls_back_to_bundled_models_dir(
    tmp_path: Path, monkeypatch
) -> None:  # noqa: ANN001
    monkeypatch.delenv("HF_HOME", raising=False)
    bundled = tmp_path / "dist" / "models" / "huggingface"
    bundled.mkdir(parents=True)

    assert _hf_home(bases=[tmp_path / "dist"]) == bundled


def test_hf_home_falls_back_to_home_cache(
    tmp_path: Path, monkeypatch
) -> None:  # noqa: ANN001
    monkeypatch.delenv("HF_HOME", raising=False)
    fake_home = tmp_path / "home"
    fake_home.mkdir()
    monkeypatch.setattr(Path, "home", lambda: fake_home)

    assert _hf_home(bases=[tmp_path / "empty"]) == (
        fake_home / ".rhizome" / "models" / "huggingface"
    )


def test_bundled_hf_home_skips_missing_dirs(tmp_path: Path) -> None:
    assert _bundled_hf_home([tmp_path / "missing"]) is None
    assert _bundled_hf_home([]) is None


def test_default_bases_frozen_mode(tmp_path: Path, monkeypatch) -> None:  # noqa: ANN001
    exe = tmp_path / "dist" / "extractor" / "rhizome-extract.exe"
    monkeypatch.setattr(sys, "frozen", True, raising=False)
    monkeypatch.setattr(sys, "executable", str(exe))

    bases = _default_bases()
    assert bases[0] == tmp_path / "dist" / "extractor"
    assert bases[1] == tmp_path / "dist"


def test_default_bases_dev_mode_points_at_repo_root() -> None:
    bases = _default_bases()
    # 开发模式：__file__ 上三级 = apps/extractor 包根
    assert bases == [Path(__file__).resolve().parent.parent]


def test_cli_module_is_invokable_via_subprocess(tmp_path: Path) -> None:
    """-m 方式调用（桌面端 uv run 场景的冒烟验证）。"""
    page = make_html(tmp_path / "a.html")

    result = subprocess.run(
        [sys.executable, "-m", "rhizome_extract.cli", str(page)],
        capture_output=True,
        encoding="utf-8",
        env={**os.environ, "PYTHONIOENCODING": "utf-8"},
        check=False,
    )

    assert result.returncode == 0, result.stderr
    assert "# 导入指南" in result.stdout


def test_main_prints_extracted_text_to_stdout(tmp_path: Path, capsys) -> None:  # noqa: ANN001
    page = make_html(tmp_path / "a.html")

    code = main([str(page)])

    captured = capsys.readouterr()
    assert code == 0
    assert captured.err == ""
    assert "# 导入指南" in captured.out
    assert captured.out.endswith("\n")


def test_main_pdf_roundtrip(tmp_path: Path, capsys) -> None:  # noqa: ANN001
    document = make_pdf(tmp_path / "a.pdf")

    code = main([str(document)])

    captured = capsys.readouterr()
    assert code == 0
    assert "PDF extraction check" in captured.out


def test_main_unsupported_type_exits_with_code_2(tmp_path: Path, capsys) -> None:  # noqa: ANN001
    bogus = tmp_path / "a.xyz"
    bogus.write_bytes(b"???")

    code = main([str(bogus)])

    captured = capsys.readouterr()
    assert code == 2
    assert "unsupported file type" in captured.err
    assert captured.out == ""


def test_main_extraction_failure_exits_with_code_1(tmp_path: Path, capsys) -> None:  # noqa: ANN001
    # 扩展名受支持但内容不是合法文件
    fake = tmp_path / "a.pdf"
    fake.write_bytes(b"not a pdf")

    code = main([str(fake)])

    captured = capsys.readouterr()
    assert code == 1
    assert "抽取失败" in captured.err
