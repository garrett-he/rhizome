"""Audio validation.

Audio files share the transcription pipeline with videos (faster-whisper
decodes them via PyAV); the type only matters for scanning and messages.
"""

from pathlib import Path

AUDIO_EXTENSIONS = frozenset(
    {
        ".aac",
        ".aiff",
        ".amr",
        ".flac",
        ".m4a",
        ".mp3",
        ".ogg",
        ".opus",
        ".wav",
        ".wma",
    }
)


def is_audio(path: Path) -> bool:
    """Return True if the path has an audio file extension (case-insensitive)."""
    return path.suffix.lower() in AUDIO_EXTENSIONS
