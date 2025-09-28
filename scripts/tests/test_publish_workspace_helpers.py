"""Unit tests for publish_workspace helper functions."""

from __future__ import annotations

import typing as typ

from tomlkit import array, dumps, parse
from tomlkit.items import Array

if typ.TYPE_CHECKING:
    from pathlib import Path
    from types import ModuleType


def test_get_valid_workspace_members_returns_array(
    publish_workspace_module: ModuleType,
) -> None:
    """Return the array intact when the manifest already uses tomlkit arrays."""
    document = parse(
        "\n".join(
            (
                "[workspace]",
                'members = ["crates/rstest-bdd", "examples/todo-cli"]',
            )
        )
    )

    result = publish_workspace_module._get_valid_workspace_members(document)

    assert isinstance(result, Array)
    assert [*result] == ["crates/rstest-bdd", "examples/todo-cli"]


def test_get_valid_workspace_members_rebuilds_list(
    publish_workspace_module: ModuleType,
) -> None:
    """Convert list-based members back into a tomlkit array."""
    document = parse('[workspace]\nresolver = "2"\n')
    document["workspace"]["members"] = [
        "crates/rstest-bdd",
        "examples/todo-cli",
    ]

    result = publish_workspace_module._get_valid_workspace_members(document)

    assert isinstance(result, Array)
    assert document["workspace"]["members"] is result
    assert [*result] == ["crates/rstest-bdd", "examples/todo-cli"]


def test_get_valid_workspace_members_handles_missing_members(
    publish_workspace_module: ModuleType,
) -> None:
    """Return ``None`` when the members array is absent."""
    document = parse('[workspace]\nresolver = "2"\n')

    result = publish_workspace_module._get_valid_workspace_members(document)

    assert result is None


def test_get_valid_workspace_members_handles_missing_workspace(
    publish_workspace_module: ModuleType,
) -> None:
    """Return ``None`` when the manifest lacks a workspace table."""
    document = parse('[package]\nname = "demo"\n')

    result = publish_workspace_module._get_valid_workspace_members(document)

    assert result is None


def test_filter_workspace_members_removes_ineligible_entries(
    publish_workspace_module: ModuleType,
) -> None:
    """Strip members that do not correspond to publishable crates."""
    members = array()
    members.extend(["crates/rstest-bdd", "examples/todo-cli", 42])

    changed = publish_workspace_module._filter_workspace_members(members)

    assert changed is True
    assert [*members] == ["crates/rstest-bdd"]


def test_filter_workspace_members_retains_publishable_entries(
    publish_workspace_module: ModuleType,
) -> None:
    """Leave publishable crate entries untouched."""
    members = array()
    members.extend(["crates/rstest-bdd", "crates/rstest-bdd-macros"])

    changed = publish_workspace_module._filter_workspace_members(members)

    assert changed is False
    assert [*members] == [
        "crates/rstest-bdd",
        "crates/rstest-bdd-macros",
    ]


def test_should_include_more_lines_continues_within_limit(
    publish_workspace_module: ModuleType,
) -> None:
    """Continue extraction while the window remains within the section."""
    lines = ["[workspace]", "members = []", ""]

    assert publish_workspace_module._should_include_more_lines(lines, 2, 0) is True


def test_should_include_more_lines_stops_at_new_section(
    publish_workspace_module: ModuleType,
) -> None:
    """Stop extraction when the next table header is encountered."""
    lines = ["[workspace]", "", "[dependencies]"]

    assert publish_workspace_module._should_include_more_lines(lines, 2, 0) is False


def test_ensure_members_array_returns_existing_array(
    publish_workspace_module: ModuleType,
) -> None:
    """Return the original array when already using tomlkit arrays."""
    members = array()
    members.extend(["crates/rstest-bdd"])

    result = publish_workspace_module._ensure_members_array(
        {"members": members}, members
    )

    assert result is members


def test_ensure_members_array_converts_lists(
    publish_workspace_module: ModuleType,
) -> None:
    """Coerce plain lists into tomlkit arrays."""
    workspace: dict[str, object] = {}
    members_list = ["crates/rstest-bdd"]

    result = publish_workspace_module._ensure_members_array(workspace, members_list)

    assert isinstance(result, Array)
    assert workspace["members"] is result
    assert list(result) == members_list


def test_ensure_members_array_rejects_invalid_types(
    publish_workspace_module: ModuleType,
) -> None:
    """Ignore unsupported member representations."""
    assert (
        publish_workspace_module._ensure_members_array({"members": "demo"}, "demo")
        is None
    )


def test_convert_list_to_array_assigns_array(
    publish_workspace_module: ModuleType,
) -> None:
    """Replace the raw list with a tomlkit array."""
    workspace: dict[str, object] = {}
    members_list = ["crates/rstest-bdd", "examples/todo-cli"]

    array_members = publish_workspace_module._convert_list_to_array(
        workspace, members_list
    )

    assert isinstance(array_members, Array)
    assert workspace["members"] is array_members
    assert list(array_members) == members_list


def test_should_write_manifest_requires_changes(
    publish_workspace_module: ModuleType,
) -> None:
    """Skip writing the manifest when nothing changed."""
    document = parse("[workspace]\nmembers = []\n")

    assert (
        publish_workspace_module._should_write_manifest(
            changed=False, document=document
        )
        is False
    )


def test_should_write_manifest_requires_workspace_section(
    publish_workspace_module: ModuleType,
) -> None:
    """Do not persist manifests lacking a workspace table."""
    document = parse('[package]\nname = "demo"\n')

    assert (
        publish_workspace_module._should_write_manifest(changed=True, document=document)
        is False
    )


def test_should_write_manifest_accepts_updates(
    publish_workspace_module: ModuleType,
) -> None:
    """Persist manifests when workspace metadata changed."""
    document = parse("[workspace]\nmembers = []\n")

    assert (
        publish_workspace_module._should_write_manifest(changed=True, document=document)
        is True
    )


def test_format_multiline_members_if_needed_enables_multiline(
    publish_workspace_module: ModuleType,
) -> None:
    """Ensure multiline arrays remain formatted over multiple lines."""
    document = parse(
        "\n".join(
            (
                "[workspace]",
                "members = [",
                '  "crates/rstest-bdd",',
                '  "examples/todo-cli"',
                "]",
            )
        )
    )
    members = publish_workspace_module._get_valid_workspace_members(document)
    assert members is not None

    publish_workspace_module._format_multiline_members_if_needed(members)

    assert members.is_multiline() is True


def test_format_multiline_members_if_needed_leaves_inline_arrays(
    publish_workspace_module: ModuleType,
) -> None:
    """Keep inline arrays untouched when no newline is present."""
    members = array()
    members.extend(["crates/rstest-bdd", "crates/rstest-bdd-macros"])

    publish_workspace_module._format_multiline_members_if_needed(members)

    assert members.is_multiline() is False


def test_write_manifest_if_changed_skips_write(
    publish_workspace_module: ModuleType,
    tmp_path: Path,
) -> None:
    """Do not rewrite manifests when members remain unchanged."""
    document = parse('[workspace]\nmembers = ["crates/rstest-bdd"]\n')
    members = publish_workspace_module._get_valid_workspace_members(document)
    manifest = tmp_path / "Cargo.toml"
    manifest.write_text("original", encoding="utf-8")

    publish_workspace_module._write_manifest_if_changed(
        document=document,
        manifest=manifest,
        changed=False,
        members=members,
    )

    assert manifest.read_text(encoding="utf-8") == "original"


def test_write_manifest_if_changed_persists_updates(
    publish_workspace_module: ModuleType,
    tmp_path: Path,
) -> None:
    """Persist the manifest when pruning mutates the members array."""
    document = parse(
        "\n".join(
            (
                "[workspace]",
                "members = [",
                '  "crates/rstest-bdd",',
                '  "examples/todo-cli"',
                "]",
            )
        )
    )
    members = publish_workspace_module._get_valid_workspace_members(document)
    publish_workspace_module._filter_workspace_members(members)
    manifest = tmp_path / "Cargo.toml"
    manifest.write_text("stale", encoding="utf-8")

    publish_workspace_module._write_manifest_if_changed(
        document=document,
        manifest=manifest,
        changed=True,
        members=members,
    )

    expected = dumps(document)
    if not expected.endswith("\n"):
        expected = f"{expected}\n"
    assert manifest.read_text(encoding="utf-8") == expected


def test_write_manifest_with_newline_appends_missing_terminator(
    publish_workspace_module: ModuleType,
    tmp_path: Path,
) -> None:
    """Ensure manifests end with a newline after serialisation."""
    document = parse('[package]\nname = "demo"\n')
    manifest = tmp_path / "Cargo.toml"

    publish_workspace_module._write_manifest_with_newline(document, manifest)

    content = manifest.read_text(encoding="utf-8")
    assert content.endswith("\n")
    assert content.count("\n") >= 2


def test_workspace_section_excerpt_returns_none(
    publish_workspace_module: ModuleType,
) -> None:
    """Return ``None`` when the manifest lacks a workspace section."""
    manifest_text = '[package]\nname = "demo"\n'

    assert publish_workspace_module._workspace_section_excerpt(manifest_text) is None


def test_workspace_section_excerpt_stops_at_next_section(
    publish_workspace_module: ModuleType,
) -> None:
    """Stop the excerpt when the next table header appears."""
    manifest_text = "\n".join(
        (
            "[package]",
            'name = "demo"',
            "",
            "[workspace]",
            'members = ["crates/rstest-bdd"]',
            "",
            "[dependencies]",
            'demo = "1"',
        )
    )

    excerpt = publish_workspace_module._workspace_section_excerpt(manifest_text)

    assert excerpt == [
        "[package]",
        "",
        "[workspace]",
        'members = ["crates/rstest-bdd"]',
    ]


def test_workspace_section_excerpt_limits_line_count(
    publish_workspace_module: ModuleType,
) -> None:
    """Limit the excerpt to eight lines following the section header."""
    manifest_text = "\n".join(
        [
            "[workspace]",
            "members = [",
            '  "a"',
            '  "b"',
            "]",
            "",
            "# comment",
            'key = "value"',
            'another = "value"',
            'final = "value"',
            "[dependencies]",
        ]
    )

    excerpt = publish_workspace_module._workspace_section_excerpt(manifest_text)

    assert excerpt == [
        "[workspace]",
        "members = [",
        '  "a"',
        '  "b"',
        "]",
        "",
        "# comment",
        'key = "value"',
    ]
