"""Unit tests for publish_workspace patch helper functions."""

from __future__ import annotations

import typing as typ

from tomlkit import parse

if typ.TYPE_CHECKING:
    from types import ModuleType


def test_should_remove_patch_section_detects_entries(
    publish_workspace_module: ModuleType,
) -> None:
    """Identify manifests that define ``[patch.crates-io]`` entries."""
    document = parse(
        "\n".join(
            (
                "[patch.crates-io]",
                'serde = { path = "../serde" }',
            )
        )
    )

    assert publish_workspace_module._should_remove_patch_section(document) is True


def test_should_remove_patch_section_ignores_invalid_tables(
    publish_workspace_module: ModuleType,
) -> None:
    """Return ``False`` when the patch section is malformed."""
    document = parse('[patch]\ncomment = "broken"\n')
    document["patch"] = "not-a-table"

    assert publish_workspace_module._should_remove_patch_section(document) is False


def test_remove_patch_section_deletes_patch_table(
    publish_workspace_module: ModuleType,
) -> None:
    """Drop the patch table when ``crates-io`` entries are removed."""
    document = parse(
        "\n".join(
            (
                "[patch.crates-io]",
                'serde = { path = "../serde" }',
            )
        )
    )
    patch_table, _ = publish_workspace_module._get_patch_crates_io_tables(document)

    publish_workspace_module._remove_patch_section(document, patch_table)

    assert "patch" not in document


def test_remove_crate_and_cleanup_preserves_remaining_entries(
    publish_workspace_module: ModuleType,
) -> None:
    """Retain the patch table when other crates remain."""
    document = parse(
        "\n".join(
            (
                "[patch.crates-io]",
                'serde = { path = "../serde" }',
                'toml = { path = "../toml" }',
            )
        )
    )
    patch_table, crates_io = publish_workspace_module._get_patch_crates_io_tables(
        document
    )

    publish_workspace_module._remove_crate_and_cleanup_empty_sections(
        document=document,
        patch_table=patch_table,
        crates_io=crates_io,
        crate="serde",
    )

    assert "serde" not in crates_io
    assert "patch" in document


def test_remove_crate_and_cleanup_removes_empty_patch(
    publish_workspace_module: ModuleType,
) -> None:
    """Delete the patch table when the final crate is removed."""
    document = parse(
        "\n".join(
            (
                "[patch.crates-io]",
                'serde = { path = "../serde" }',
            )
        )
    )
    patch_table, crates_io = publish_workspace_module._get_patch_crates_io_tables(
        document
    )

    publish_workspace_module._remove_crate_and_cleanup_empty_sections(
        document=document,
        patch_table=patch_table,
        crates_io=crates_io,
        crate="serde",
    )

    assert "patch" not in document
