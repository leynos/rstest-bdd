"""Dependency rewriting utilities for publish-time manifest adjustments.

These helpers centralise the logic that updates inter-crate dependency entries
so release automation can substitute version numbers and optional local path
references in a single place.
"""

from __future__ import annotations

import typing as typ
from pathlib import Path

from publish_patch import REPLACEMENTS, apply_replacements

__all__ = ["apply_workspace_replacements"]


def apply_workspace_replacements(
    workspace_root: Path,
    version: str,
    *,
    include_local_path: bool,
    crates: tuple[str, ...] | None = None,
) -> None:
    """Rewrite workspace dependency declarations for publish workflows."""
    workspace_root = Path(workspace_root)
    if crates is None:
        targets: typ.Final[tuple[str, ...]] = tuple(REPLACEMENTS)
    else:
        unknown = tuple(crate for crate in crates if crate not in REPLACEMENTS)
        if unknown:
            formatted = ", ".join(repr(crate) for crate in unknown)
            message = "unknown crates: " + formatted
            raise SystemExit(message)
        targets = crates
    for crate in targets:
        manifest = workspace_root / "crates" / crate / "Cargo.toml"
        apply_replacements(
            crate,
            manifest,
            version,
            include_local_path=include_local_path,
        )
