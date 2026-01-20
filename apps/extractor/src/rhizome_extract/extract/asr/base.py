"""Common types shared by ASR backends."""

from dataclasses import dataclass


@dataclass
class Segment:
    """A single transcription segment with timestamps in seconds."""

    start: float
    end: float
    text: str
