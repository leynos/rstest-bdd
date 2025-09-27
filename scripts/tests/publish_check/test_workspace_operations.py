"""Workspace export workflow tests."""

from __future__ import annotations

import contextlib
import dataclasses as dc
import typing as typ

import pytest
import tomllib
from tomlkit import parse
from tomlkit.items import Array

if typ.TYPE_CHECKING:
    from pathlib import Path
    from types import ModuleType

    from tomlkit.toml_document import TOMLDocument


def test_export_workspace_creates_manifest_copy(
    run_publish_check_module: ModuleType, tmp_path: Path
) -> None:
    """Verify exporting the workspace copies the manifest into place."""
    destination = tmp_path / "workspace"
    destination.mkdir()

    run_publish_check_module.export_workspace(destination)

    assert (destination / "Cargo.toml").exists()


def test_export_workspace_propagates_git_failure(
    monkeypatch: pytest.MonkeyPatch,
    publish_workspace_module: ModuleType,
    tmp_path: Path,
) -> None:
    """Ensure failures raised by git archive are surfaced to the caller."""

    class FakeCommand:
        def __getitem__(self, _args: object) -> FakeCommand:
            return self

        def run(
            self,
            *,
            timeout: int,
            retcode: object | None,
        ) -> tuple[int, str, str]:
            return 1, "", "archive failed"

    class FakeLocal:
        def __getitem__(self, name: str) -> FakeCommand:
            if name != "git":
                msg = f"FakeLocal expected 'git' command but received {name!r}"
                raise RuntimeError(msg)
            return FakeCommand()

        def cwd(self, _path: Path) -> contextlib.AbstractContextManager[None]:
            return contextlib.nullcontext()

    monkeypatch.setattr(publish_workspace_module, "local", FakeLocal())

    with pytest.raises(SystemExit, match="git archive failed with exit code 1"):
        publish_workspace_module.export_workspace(tmp_path)


def test_strip_patch_section_ignores_inline_comments(
    publish_workspace_module: ModuleType, tmp_path: Path
) -> None:
    """Ensure patch sections with inline comments are removed cleanly."""
    manifest = tmp_path / "Cargo.toml"
    manifest.write_text(
        "\n".join(
            (
                "[package]",
                'name = "demo"',
                'version = "0.1.0"',
                "",
                "[patch.crates-io] # remove before publish",
                'serde = { path = "../serde" }',
                "",
                "[dependencies]",
                'serde = "1"',
            )
        ),
        encoding="utf-8",
    )

    publish_workspace_module.strip_patch_section(manifest)

    assert manifest.read_text(encoding="utf-8") == (
        '[package]\nname = "demo"\nversion = "0.1.0"\n\n[dependencies]\nserde = "1"\n'
    )


def test_strip_patch_section_tolerates_hash_characters_in_strings(
    publish_workspace_module: ModuleType, tmp_path: Path
) -> None:
    """Ignore literal ``#`` characters embedded within string values."""
    manifest = tmp_path / "Cargo.toml"
    manifest.write_text(
        "\n".join(
            (
                "[patch.crates-io]",
                'demo = { git = "https://example.com/#main" }',
                "",
                "[dependencies]",
                'demo = { version = "1", registry = "crates-io" }',
            )
        ),
        encoding="utf-8",
    )

    publish_workspace_module.strip_patch_section(manifest)

    assert manifest.read_text(encoding="utf-8") == (
        '[dependencies]\ndemo = { version = "1", registry = "crates-io" }\n'
    )


@dc.dataclass(frozen=True)
class PruneTestCase:
    """Test case data for workspace member pruning scenarios."""

    name: str
    toml_content: str
    expected: list[str] | str
    description: str


@pytest.mark.parametrize(
    "test_case",
    [
        PruneTestCase(
            name="removes_non_crate_entries",
            toml_content="\n".join(
                (
                    "[workspace]",
                    "members = [",
                    '    "crates/rstest-bdd",',
                    '    "examples/todo-cli",',
                    "]",
                )
            ),
            expected=["crates/rstest-bdd"],
            description=(
                "Remove workspace members that are not part of the publishable crates."
            ),
        ),
        PruneTestCase(
            name="keeps_known_crate_names",
            toml_content="\n".join(
                (
                    "[workspace]",
                    'members = ["packages/rstest-bdd", "tools/xtask"]',
                )
            ),
            expected=["packages/rstest-bdd"],
            description=(
                "Retain crate entries even when they use alternate directory layouts."
            ),
        ),
        PruneTestCase(
            name="ignores_missing_workspace_section",
            toml_content="\n".join(
                (
                    "[package]",
                    'name = "demo"',
                    'version = "0.1.0"',
                )
            ),
            expected="\n".join(
                (
                    "[package]",
                    'name = "demo"',
                    'version = "0.1.0"',
                )
            ),
            description="Leave manifests without workspace metadata untouched.",
        ),
        PruneTestCase(
            name="ignores_missing_members_array",
            toml_content="\n".join(
                (
                    "[workspace]",
                    'resolver = "2"',
                )
            ),
            expected="\n".join(
                (
                    "[workspace]",
                    'resolver = "2"',
                )
            ),
            description="Preserve workspace tables that do not define members.",
        ),
    ],
    ids=lambda case: case.name.replace("_", "-"),
)
def test_prune_workspace_members_behaviour(
    publish_workspace_module: ModuleType,
    tmp_path: Path,
    test_case: PruneTestCase,
) -> None:
    """Exercise prune_workspace_members across expected scenarios."""
    manifest = tmp_path / "Cargo.toml"
    manifest.write_text(test_case.toml_content, encoding="utf-8")

    original = manifest.read_text(encoding="utf-8")

    publish_workspace_module.prune_workspace_members(manifest)

    content = manifest.read_text(encoding="utf-8")
    if isinstance(test_case.expected, list):
        data = tomllib.loads(content)
        assert data["workspace"]["members"] == test_case.expected
    else:
        assert content == test_case.expected == original


def test_prune_workspace_members_preserves_inline_formatting(
    publish_workspace_module: ModuleType,
    tmp_path: Path,
) -> None:
    """Retain inline array formatting and trailing newline when pruning."""
    manifest = tmp_path / "Cargo.toml"
    manifest.write_text(
        "\n".join(
            (
                "[workspace]",
                'members = ["crates/rstest-bdd", "examples/todo-cli"]',
            )
        )
        + "\n",
        encoding="utf-8",
    )

    publish_workspace_module.prune_workspace_members(manifest)

    content = manifest.read_text(encoding="utf-8")
    assert 'members = ["crates/rstest-bdd"]' in content
    assert content.endswith("\n")


def test_prune_workspace_members_handles_python_list_members(
    monkeypatch: pytest.MonkeyPatch,
    publish_workspace_module: ModuleType,
    tmp_path: Path,
) -> None:
    """Ensure list-based members are normalised to arrays before pruning."""
    manifest = tmp_path / "Cargo.toml"
    manifest.write_text(
        '[workspace]\nmembers = ["crates/rstest-bdd"]\n', encoding="utf-8"
    )

    document = parse(manifest.read_text(encoding="utf-8"))
    document["workspace"]["members"] = [
        "crates/rstest-bdd",
        "examples/todo-cli",
    ]

    def fake_parse(_text: str) -> TOMLDocument:
        return document

    monkeypatch.setattr(publish_workspace_module, "parse", fake_parse)

    publish_workspace_module.prune_workspace_members(manifest)

    data = tomllib.loads(manifest.read_text(encoding="utf-8"))
    assert data["workspace"]["members"] == ["crates/rstest-bdd"]
    assert isinstance(document["workspace"]["members"], Array)


def test_prune_workspace_members_skips_write_when_members_unchanged(
    publish_workspace_module: ModuleType,
    tmp_path: Path,
) -> None:
    """Verify manifests remain untouched when pruning produces no changes."""
    manifest = tmp_path / "Cargo.toml"
    manifest.write_text(
        '[workspace]\nmembers = ["crates/rstest-bdd", "crates/rstest-bdd-macros"]\n',
        encoding="utf-8",
    )

    original = manifest.read_text(encoding="utf-8")

    publish_workspace_module.prune_workspace_members(manifest)

    assert manifest.read_text(encoding="utf-8") == original


def test_workspace_version_error_includes_workspace_excerpt(
    publish_workspace_module: ModuleType,
    tmp_path: Path,
) -> None:
    """Confirm workspace version diagnostics display the surrounding snippet."""
    manifest = tmp_path / "Cargo.toml"
    manifest.write_text(
        "\n".join(
            (
                "[workspace]",
                'members = ["crates/rstest-bdd"]',
                "",
                "[package]",
                'name = "demo"',
            )
        ),
        encoding="utf-8",
    )

    with pytest.raises(SystemExit) as exc:
        publish_workspace_module.workspace_version(manifest)

    message = str(exc.value)
    assert "Workspace manifest excerpt" in message
    assert "    [workspace]" in message
    assert "[workspace.package]" in message
