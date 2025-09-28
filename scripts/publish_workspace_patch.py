"""Patch table management for workspace manifests prepared for publishing.

The helpers in this module encapsulate logic for stripping ``[patch.crates-io]``
entries so release pipelines can clean manifests before packaging crates.
"""

from __future__ import annotations

import typing as typ
from pathlib import Path

from publish_workspace_serialise import _write_manifest_with_newline
from tomlkit import parse

if typ.TYPE_CHECKING:
    from tomlkit.toml_document import TOMLDocument

__all__ = [
    "_get_patch_crates_io_tables",
    "_remove_crate_and_cleanup_empty_sections",
    "_remove_patch_section",
    "remove_patch_entry",
    "strip_patch_section",
]


def strip_patch_section(manifest: Path) -> None:
    """Strip the ``[patch.crates-io]`` section from ``manifest``."""
    manifest = Path(manifest)
    document = parse(manifest.read_text(encoding="utf-8"))
    patch_tables = _get_patch_crates_io_tables(document)
    if patch_tables is None:
        return

    patch_table, _ = patch_tables
    _remove_patch_section(document, patch_table)
    _write_manifest_with_newline(document, manifest)


def remove_patch_entry(manifest: Path, crate: str) -> None:
    """Remove the ``crate`` entry from the root ``[patch.crates-io]`` table."""
    manifest = Path(manifest)
    document = parse(manifest.read_text(encoding="utf-8"))
    patch_tables = _get_patch_crates_io_tables(document)
    if patch_tables is None:
        return

    patch_table, crates_io = patch_tables
    if crate not in crates_io:
        return

    _remove_crate_and_cleanup_empty_sections(
        document=document,
        patch_table=patch_table,
        crates_io=crates_io,
        crate=crate,
    )
    _write_manifest_with_newline(document, manifest)


def _get_patch_crates_io_tables(
    document: TOMLDocument,
) -> tuple[dict[str, typ.Any], dict[str, typ.Any]] | None:
    """Return the patch and crates-io tables when both are present."""
    patch_table = document.get("patch")
    if patch_table is None:
        return None

    crates_io = patch_table.get("crates-io")
    if crates_io is None:
        return None

    return (
        typ.cast("dict[str, typ.Any]", patch_table),
        typ.cast("dict[str, typ.Any]", crates_io),
    )


def _remove_patch_section(
    document: TOMLDocument, patch_table: dict[str, typ.Any]
) -> None:
    """Remove the entire ``[patch.crates-io]`` table from ``document``."""
    patch_table.pop("crates-io", None)
    if not patch_table:
        del document["patch"]


def _remove_crate_and_cleanup_empty_sections(
    *,
    document: TOMLDocument,
    patch_table: dict[str, typ.Any],
    crates_io: dict[str, typ.Any],
    crate: str,
) -> None:
    """Remove ``crate`` from the patch section and drop empty tables."""
    del crates_io[crate]
    if not crates_io:
        del patch_table["crates-io"]
    if not patch_table:
        del document["patch"]
