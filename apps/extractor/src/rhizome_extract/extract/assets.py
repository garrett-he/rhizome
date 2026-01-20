"""Shared asset-directory conventions for converters that extract resources."""

from pathlib import Path


def assets_dir_for(output: Path) -> Path:
    """Return the asset directory that accompanies one output file."""
    return output.parent / f"{output.stem}_files"
