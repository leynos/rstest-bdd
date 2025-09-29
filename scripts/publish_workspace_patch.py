"""Patch table management for workspace manifests prepared for publishing.

The helpers in this module encapsulate logic for stripping ``[patch.crates-io]``
entries so release pipelines can clean manifests before packaging crates.
"""

from __future__ import annotations

import collections.abc as cabc
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
    "_should_remove_patch_section",
    "remove_patch_entry",
    "strip_patch_section",
]


def strip_patch_section(manifest: Path) -> None:
    """Strip the ``[patch.crates-io]`` section from ``manifest``."""
    manifest = Path(manifest)
    document = parse(manifest.read_text(encoding="utf-8"))
    if not _should_remove_patch_section(document):
        return

    patch_tables = _get_patch_crates_io_tables(document)
    if patch_tables is None:  # pragma: no cover - defensive
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
) -> tuple[cabc.MutableMapping[str, typ.Any], cabc.MutableMapping[str, typ.Any]] | None:
    """Return the patch and crates-io tables when both are present."""
    patch_table = document.get("patch")
    if not isinstance(patch_table, cabc.MutableMapping):
        return None

    patch_mapping = typ.cast("cabc.MutableMapping[str, typ.Any]", patch_table)

    crates_io = patch_mapping.get("crates-io")
    if not isinstance(crates_io, cabc.MutableMapping):
        return None

    return (
        patch_mapping,
        typ.cast("cabc.MutableMapping[str, typ.Any]", crates_io),
    )


def _should_remove_patch_section(document: TOMLDocument) -> bool:
    """Return ``True`` when the patch section contains ``crates-io`` entries."""
    patch_tables = _get_patch_crates_io_tables(document)
    if patch_tables is None:
        return False

    _, crates_io = patch_tables
    return bool(crates_io)


def _remove_patch_section(
    document: TOMLDocument, patch_table: cabc.MutableMapping[str, typ.Any]
) -> None:
    """Remove the entire ``[patch.crates-io]`` table from ``document``."""
    patch_table.pop("crates-io", None)
    if not patch_table:
        del document["patch"]


def _remove_crate_and_cleanup_empty_sections(
    *,
    document: TOMLDocument,
    patch_table: cabc.MutableMapping[str, typ.Any],
    crates_io: cabc.MutableMapping[str, typ.Any],
    crate: str,
) -> None:
    """Remove ``crate`` from the patch section and drop empty tables."""
    del crates_io[crate]
    if not crates_io:
        del patch_table["crates-io"]
    if not patch_table:
        del document["patch"]
