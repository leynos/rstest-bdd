"""Unit tests for publish_workspace helper functions."""

from __future__ import annotations

import importlib.util
import sys
import typing as typ
from pathlib import Path

import pytest
from tomlkit import array, dumps, parse
from tomlkit.items import Array

if typ.TYPE_CHECKING:
    from types import ModuleType

SCRIPTS_DIR = Path(__file__).resolve().parents[1]
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))


def _load_publish_workspace_module() -> ModuleType:
    spec = importlib.util.spec_from_file_location(
        "publish_workspace", SCRIPTS_DIR / "publish_workspace.py"
    )
    if spec is None or spec.loader is None:  # pragma: no cover - defensive guard
        message = "publish_workspace module could not be loaded"
        raise RuntimeError(message)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


@pytest.fixture(scope="module")
def publish_workspace_module() -> ModuleType:
    """Provide the publish_workspace module for helper unit tests."""
    return _load_publish_workspace_module()


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
