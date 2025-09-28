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
    targets: typ.Final[tuple[str, ...]] = REPLACEMENTS if crates is None else crates
    for crate in targets:
        if crate not in REPLACEMENTS:
            continue
        manifest = workspace_root / "crates" / crate / "Cargo.toml"
        apply_replacements(
            crate,
            manifest,
            version,
            include_local_path=include_local_path,
        )
