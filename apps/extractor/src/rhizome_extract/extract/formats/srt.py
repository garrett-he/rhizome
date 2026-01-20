"""SubRip (SRT) subtitle rendering.

Line wrapping follows the Netflix subtitle convention: a display width
budget where full-width (CJK) characters count as 2 and half-width
characters count as 1, with the default budget of 42 per line. Segments
whose wrapped text exceeds the per-cue line limit are split into
multiple cues with proportionally redistributed times, so no single
cue covers the screen for too long.
"""

import unicodedata
from collections.abc import Iterable

from rhizome_extract.extract.asr.base import Segment

DEFAULT_LINE_WIDTH = 42
DEFAULT_MAX_LINES = 2

# Closing punctuation that must not start a wrapped line; when a wrap
# would place one at the start of a line it is pulled up to the previous
# line instead (possibly exceeding the width budget slightly).
CLOSING_PUNCTUATION = frozenset("，。！？；：、）》〉】」』”’…,.!?;:)]}'\"")  # noqa: RUF001


def _char_width(ch: str) -> int:
    """Return the display width of a single character (2 for full-width)."""
    if ch == "\n":
        return 0
    return 2 if unicodedata.east_asian_width(ch) in ("W", "F") else 1


def display_width(text: str) -> int:
    """Return the display width of text (full-width counts as 2)."""
    return sum(_char_width(ch) for ch in text)


def _tokens(text: str) -> list[str]:
    """Split text into wrap tokens.

    Each full-width character is its own token (breakable anywhere);
    runs of half-width characters form word tokens kept intact; spaces
    are standalone tokens.
    """
    tokens: list[str] = []
    buf = ""
    for ch in text:
        if ch == " ":
            if buf:
                tokens.append(buf)
                buf = ""
            tokens.append(" ")
        elif _char_width(ch) == 2:
            if buf:
                tokens.append(buf)
                buf = ""
            tokens.append(ch)
        else:
            buf += ch
    if buf:
        tokens.append(buf)
    return tokens


def wrap_text(text: str, width: int = DEFAULT_LINE_WIDTH) -> str:
    """Wrap text so each line stays within the display width budget.

    Half-width words are never split; wrapped lines never start with
    closing punctuation (the punctuation is pulled up to the previous
    line, which may exceed the budget by one token).
    """
    lines: list[str] = []
    line = ""

    def flush() -> None:
        nonlocal line
        if line:
            lines.append(line.rstrip())
            line = ""

    for tok in _tokens(text):
        if tok == " ":
            if line and not line.endswith(" ") and display_width(line) + 1 <= width:
                line += tok
            continue
        if line and display_width(line) + display_width(tok) > width:
            if tok in CLOSING_PUNCTUATION:
                line += tok
                flush()
            else:
                flush()
                line = tok
        else:
            line += tok
    flush()
    return "\n".join(lines)


def format_srt_time(seconds: float) -> str:
    """Format seconds as an SRT timeline value (hh:mm:ss,mmm)."""
    total_ms = round(seconds * 1000)
    hours, remainder = divmod(total_ms, 3_600_000)
    minutes, remainder = divmod(remainder, 60_000)
    secs, ms = divmod(remainder, 1000)
    return f"{hours:02d}:{minutes:02d}:{secs:02d},{ms:03d}"


def format_srt(
    segments: Iterable[Segment],
    *,
    line_width: int = DEFAULT_LINE_WIDTH,
    max_lines: int = DEFAULT_MAX_LINES,
) -> str:
    """Render segments in SubRip (SRT) subtitle format.

    Non-empty segments are numbered sequentially; empty segments are
    skipped and do not consume a number. Segment text is wrapped so that
    each line stays within ``line_width`` display columns, and cues hold
    at most ``max_lines`` lines: a segment with more wrapped lines is
    split into several cues whose times are redistributed proportionally
    to the display width of each chunk. Zero-duration segments are never
    split (proportional times would be identical anyway).
    """
    entries: list[str] = []
    index = 0
    for seg in segments:
        text = seg.text.strip()
        if not text:
            continue
        lines = wrap_text(text, line_width).split("\n")
        if seg.end <= seg.start:
            chunks = ["\n".join(lines)]
        else:
            chunks = ["\n".join(lines[i : i + max_lines]) for i in range(0, len(lines), max_lines)]
        weights = [display_width(chunk) for chunk in chunks]
        total = sum(weights)
        start = seg.start
        cumulative = 0
        for chunk, weight in zip(chunks, weights, strict=True):
            index += 1
            cumulative += weight
            end = seg.end if cumulative >= total else start + (seg.end - start) * cumulative / total
            timeline = f"{format_srt_time(start)} --> {format_srt_time(end)}"
            entries.append(f"{index}\n{timeline}\n{chunk}")
            start = end
    return "\n\n".join(entries)
