"""Text recognition with RapidOCR (ONNX Runtime backend).

Unlike ASR, OCR output has no timeline, so it does not yield Segment
objects; it returns plain text directly.
"""

from functools import lru_cache
from pathlib import Path
from typing import TYPE_CHECKING, cast

from numpy import ndarray
from rapidocr import RapidOCR

if TYPE_CHECKING:
    from rapidocr.utils.output import RapidOCROutput


@lru_cache(maxsize=1)
def _load_engine() -> RapidOCR:
    """Create and cache a single RapidOCR engine instance."""
    return RapidOCR()


def _sort_and_join(txts: tuple[str, ...] | None, boxes: ndarray | None) -> str:
    """Join recognized texts one per line, sorted top-to-bottom by box top edge.

    ``boxes`` is an ``(N, 4, 2)`` array of corner coordinates aligned with
    ``txts``. When boxes are missing or misaligned the detection order is
    kept as-is.
    """
    if not txts:
        return ""
    if boxes is None or len(boxes) != len(txts):
        return "\n".join(txts)
    order = sorted(range(len(txts)), key=lambda index: float(boxes[index][:, 1].min()))
    return "\n".join(txts[index] for index in order)


def recognize(path: Path) -> str:
    """Recognize the text in an image file.

    Each detected text block becomes one line, ordered top-to-bottom by
    the block's top edge. Returns an empty string when nothing is found.
    """
    result = cast("RapidOCROutput", _load_engine()(str(path)))
    return _sort_and_join(result.txts, result.boxes)
