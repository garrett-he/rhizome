"""Input media handling, one module per media type.

Adding a new media type means adding a module here with its extension
set and a predicate, then registering it in the dispatch registry in
:mod:`rhizome_extract.extract` (``_REGISTRY``). :func:`is_supported`
below delegates to that registry, so support checks and dispatch can
never drift apart. The conversion pipeline in cli.py is media-type
agnostic and must stay that way.
"""

from pathlib import Path

__all__ = ["is_supported"]


def is_supported(path: Path) -> bool:
    """Return True if any registered media type claims the path's extension."""
    # 延迟导入：extract 包初始化时会加载本包的子模块，这里不能在模块层反向依赖。
    from rhizome_extract.extract import is_supported as _registry_supports

    return _registry_supports(path)
