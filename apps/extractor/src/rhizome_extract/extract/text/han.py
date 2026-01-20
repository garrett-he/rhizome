"""Chinese script conversion between simplified and traditional."""

from enum import StrEnum
from functools import lru_cache

from opencc import OpenCC


class HanVariant(StrEnum):
    """Chinese script variants supported by the converter."""

    AUTO = "auto"
    SIMPLIFIED = "simplified"
    TRADITIONAL = "traditional"


@lru_cache(maxsize=2)
def _converter(config: str) -> OpenCC:
    """Return a cached OpenCC converter for the given config."""
    return OpenCC(config)


def convert_han(text: str, variant: HanVariant) -> str:
    """Convert text to the requested Chinese script variant.

    ``HanVariant.AUTO`` returns the text unchanged; SIMPLIFIED and
    TRADITIONAL convert via OpenCC.
    """
    if variant is HanVariant.SIMPLIFIED:
        return _converter("t2s").convert(text)
    if variant is HanVariant.TRADITIONAL:
        return _converter("s2t").convert(text)
    return text
