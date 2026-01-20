"""HTML validation and local-asset extraction.

Detection is extension-based (case-insensitive). Rendering to Markdown
is handled by the shared HTML-to-Markdown renderer in
:mod:`rhizome_extract.extract.formats.markdown`; images referenced by local paths are
copied into an asset directory next to the output file and their
``src`` rewritten, following the same ``{stem}_files/`` convention as
docx, pptx, and pdf.
"""

import re
import shutil
from pathlib import Path
from urllib.parse import unquote

from bs4 import BeautifulSoup

HTML_EXTENSIONS = frozenset({".htm", ".html"})

# src="..." values that do not point at a local file.
_REMOTE_SRC_PATTERN = re.compile(r"^(?:[a-zA-Z][a-zA-Z0-9+.-]*:|//)")


def is_html(path: Path) -> bool:
    """Return True if the path has an HTML file extension (case-insensitive)."""
    return path.suffix.lower() in HTML_EXTENSIONS


def _is_local_reference(src: str) -> bool:
    """Return True if an img src looks like a local filesystem path."""
    return not _REMOTE_SRC_PATTERN.match(src)


def _candidate_path(src: str, source: Path) -> Path | None:
    """Resolve an img src against the HTML file's directory.

    Returns None for absolute paths or paths escaping the HTML file's
    directory: HTML is untrusted input, so only assets inside the source
    tree may be copied.
    """
    local = unquote(src.replace("\\", "/"))
    candidate = Path(local)
    resolved = (source.parent / candidate).resolve()
    root = source.parent.resolve()
    if resolved != root and not resolved.is_relative_to(root):
        return None
    return resolved


def _unique_name(basename: str, taken: set[str]) -> str:
    """Return an asset filename derived from basename that is unused in this run.

    Names already present in the asset directory (from a previous run)
    are reused; the copy overwrites them.
    """
    if basename not in taken:
        taken.add(basename)
        return basename
    stem, dot, suffix = basename.rpartition(".")
    prefix = f"{stem}_" if dot else f"{basename}_"
    extension = f".{suffix}" if dot else ""
    index = 0
    name = basename
    while name in taken:
        index += 1
        name = f"{prefix}{index}{extension}"
    taken.add(name)
    return name


def copy_local_images(html: str, source: Path, assets_dir: Path) -> str:
    """Copy images referenced by local paths into assets_dir and rewrite their src.

    Relative paths are resolved against the HTML file's directory and
    Windows-style separators are accepted. Remote URLs and data URIs are
    left untouched, as are references to files that do not exist or that
    resolve outside the HTML file's directory. The
    copied files keep their basename, with whitespace replaced by
    underscores and a numeric suffix appended on basename conflicts.
    Returns the rewritten HTML.
    """
    soup = BeautifulSoup(html, "html.parser")
    taken: set[str] = set()
    for img in soup.find_all("img"):
        src = img.get("src")
        if not isinstance(src, str) or not _is_local_reference(src):
            continue
        candidate = _candidate_path(src, source)
        if candidate is None or not candidate.is_file():
            continue
        assets_dir.mkdir(parents=True, exist_ok=True)
        name = _unique_name(candidate.name.replace(" ", "_"), taken)
        shutil.copyfile(candidate, assets_dir / name)
        img["src"] = f"{assets_dir.name}/{name}"
    return str(soup)
