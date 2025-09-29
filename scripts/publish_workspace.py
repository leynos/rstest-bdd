"""Facade module for publish workspace utilities.

This module preserves the historical import surface while delegating the
implementation to focused helper modules. Callers should prefer the re-exported
functions documented in the respective modules for clarity.
"""

from __future__ import annotations

import typing as typ

import publish_workspace_archive as _archive
import publish_workspace_dependencies as _dependencies
import publish_workspace_members as _members
import publish_workspace_patch as _patch
import publish_workspace_serialise as _serialise
import publish_workspace_versioning as _versioning
from plumbum import local as _default_local
from tomlkit import parse as _default_parse

if typ.TYPE_CHECKING:
    from pathlib import Path

local = _default_local
parse = _default_parse


def export_workspace(destination: Path) -> None:
    """Dispatch to the archive helper while respecting runtime monkeypatches."""
    _archive.local = local
    _archive.export_workspace(destination)


def apply_workspace_replacements(
    workspace_root: Path,
    version: str,
    *,
    include_local_path: bool,
    crates: tuple[str, ...] | None = None,
) -> None:
    """Proxy dependency rewriting through the dedicated module."""
    _dependencies.apply_workspace_replacements(
        workspace_root,
        version,
        include_local_path=include_local_path,
        crates=crates,
    )


def prune_workspace_members(manifest: Path) -> None:
    """Filter workspace members using the current ``parse`` implementation."""
    _members.parse = parse
    _members.prune_workspace_members(manifest)


def strip_patch_section(manifest: Path) -> None:
    """Remove the patch section using the current ``parse`` implementation."""
    _patch.parse = parse
    _patch.strip_patch_section(manifest)


def remove_patch_entry(manifest: Path, crate: str) -> None:
    """Remove a single patch entry using the current ``parse`` implementation."""
    _patch.parse = parse
    _patch.remove_patch_entry(manifest, crate)


def workspace_version(manifest: Path) -> str:
    """Expose the workspace version helper."""
    return _versioning.workspace_version(manifest)


PUBLISHABLE_CRATES: typ.Final[tuple[str, ...]] = _members.PUBLISHABLE_CRATES
_get_valid_workspace_members = _members._get_valid_workspace_members
_ensure_members_array = _members._ensure_members_array
_convert_list_to_array = _members._convert_list_to_array
_filter_workspace_members = _members._filter_workspace_members
_should_write_manifest = _members._should_write_manifest
_format_multiline_members_if_needed = _members._format_multiline_members_if_needed
_write_manifest_if_changed = _members._write_manifest_if_changed
_get_patch_crates_io_tables = _patch._get_patch_crates_io_tables
_should_remove_patch_section = _patch._should_remove_patch_section
_remove_patch_section = _patch._remove_patch_section
_remove_crate_and_cleanup_empty_sections = (
    _patch._remove_crate_and_cleanup_empty_sections
)
_write_manifest_with_newline = _serialise._write_manifest_with_newline
_workspace_section_excerpt = _versioning._workspace_section_excerpt
_find_workspace_section_index = _versioning._find_workspace_section_index
_extract_section_lines = _versioning._extract_section_lines
_should_include_more_lines = _versioning._should_include_more_lines

__all__ = [
    "PUBLISHABLE_CRATES",
    "_convert_list_to_array",
    "_ensure_members_array",
    "_extract_section_lines",
    "_filter_workspace_members",
    "_find_workspace_section_index",
    "_format_multiline_members_if_needed",
    "_get_patch_crates_io_tables",
    "_get_valid_workspace_members",
    "_remove_crate_and_cleanup_empty_sections",
    "_remove_patch_section",
    "_should_include_more_lines",
    "_should_remove_patch_section",
    "_should_write_manifest",
    "_workspace_section_excerpt",
    "_write_manifest_if_changed",
    "_write_manifest_with_newline",
    "apply_workspace_replacements",
    "export_workspace",
    "local",
    "parse",
    "prune_workspace_members",
    "remove_patch_entry",
    "strip_patch_section",
    "workspace_version",
]
