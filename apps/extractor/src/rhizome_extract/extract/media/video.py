"""Video validation.

Detection is extension-based (case-insensitive). Decoding of the audio
track is handled by the ASR backend itself (faster-whisper decodes video
files via PyAV), so no external ffmpeg dependency is required.
"""

from pathlib import Path

VIDEO_EXTENSIONS = frozenset(
    {
        ".3gp",
        ".avi",
        ".flv",
        ".m4v",
        ".mkv",
        ".mov",
        ".mp4",
        ".mpg",
        ".mpeg",
        ".ts",
        ".webm",
        ".wmv",
    }
)


def is_video(path: Path) -> bool:
    """Return True if the path has a video file extension (case-insensitive)."""
    return path.suffix.lower() in VIDEO_EXTENSIONS
