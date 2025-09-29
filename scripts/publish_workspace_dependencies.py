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
    """Rewrite workspace dependency declarations for publish workflows.

    Parameters
    ----------
    workspace_root : Path
        Root directory of the rstest-bdd workspace whose manifests are rewritten.
    version : str
        Version string applied to patched dependency entries.
    include_local_path : bool
        Toggle whether rewritten dependencies retain their relative path entries.
    crates : tuple[str, ...] | None, optional
        Subset of crates to update; default rewrites every crate with
        replacements.

    Returns
    -------
    None
        All matching manifests are rewritten in place.

    Raises
    ------
    SystemExit
        Raised when any supplied crate lacks a replacement configuration or a
        manifest cannot be rewritten due to a missing replacement entry.

    Examples
    --------
    >>> from pathlib import Path
    >>> apply_workspace_replacements(Path("."), "1.2.3", include_local_path=False)
    """
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
