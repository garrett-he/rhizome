"""Speech recognition with faster-whisper (CTranslate2 backend)."""

from collections.abc import Iterator
from functools import lru_cache
from pathlib import Path

from faster_whisper import WhisperModel

from rhizome_extract.extract.asr.base import Segment

DEFAULT_MODEL_SIZE = "base"


@lru_cache(maxsize=4)
def _load_model(model_size: str) -> WhisperModel:
    """Load and cache a CPU int8 WhisperModel by size."""
    return WhisperModel(model_size, device="cpu", compute_type="int8")


def transcribe(
    audio: Path,
    *,
    model_size: str = DEFAULT_MODEL_SIZE,
    language: str | None = None,
) -> Iterator[Segment]:
    """Transcribe an audio or video file, yielding segments with timestamps.

    ``language`` is an ISO language code (e.g. 'zh'); None lets the model
    auto-detect. Silent stretches are filtered with VAD to avoid
    hallucinated text in the transcript.
    """
    model = _load_model(model_size)
    segments, _info = model.transcribe(str(audio), language=language, vad_filter=True)
    for seg in segments:
        yield Segment(start=seg.start, end=seg.end, text=seg.text.strip())
