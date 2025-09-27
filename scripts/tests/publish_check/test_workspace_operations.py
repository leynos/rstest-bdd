"""Workspace export workflow tests."""

from __future__ import annotations

import contextlib
import typing as typ
from typing import List

import pytest
import tomllib

if typ.TYPE_CHECKING:
    from pathlib import Path
    from types import ModuleType


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


@pytest.mark.parametrize(
    ("case_name", "toml_content", "expected", "description"),
    [
        pytest.param(
            "removes_non_crate_entries",
            "\n".join(
                (
                    "[workspace]",
                    "members = [",
                    '    "crates/rstest-bdd",',
                    '    "examples/todo-cli",',
                    "]",
                )
            ),
            ["crates/rstest-bdd"],
            "Remove workspace members that are not part of the publishable crates.",
            id="removes-non-crate-entries",
        ),
        pytest.param(
            "keeps_known_crate_names",
            "\n".join(
                (
                    "[workspace]",
                    'members = ["packages/rstest-bdd", "tools/xtask"]',
                )
            ),
            ["packages/rstest-bdd"],
            "Retain crate entries even when they use alternate directory layouts.",
            id="keeps-known-crate-names",
        ),
        pytest.param(
            "ignores_missing_workspace_section",
            "\n".join(
                (
                    "[package]",
                    'name = "demo"',
                    'version = "0.1.0"',
                )
            ),
            "\n".join(
                (
                    "[package]",
                    'name = "demo"',
                    'version = "0.1.0"',
                )
            ),
            "Leave manifests without workspace metadata untouched.",
            id="ignores-missing-workspace-section",
        ),
        pytest.param(
            "ignores_missing_members_array",
            "\n".join(
                (
                    "[workspace]",
                    'resolver = "2"',
                )
            ),
            "\n".join(
                (
                    "[workspace]",
                    'resolver = "2"',
                )
            ),
            "Preserve workspace tables that do not define members.",
            id="ignores-missing-members-array",
        ),
    ],
)
def test_prune_workspace_members_behaviour(
    publish_workspace_module: ModuleType,
    tmp_path: Path,
    request: pytest.FixtureRequest,
    case_name: str,
    toml_content: str,
    expected: List[str] | str,
    description: str,
) -> None:
    """Exercise prune_workspace_members across expected scenarios."""
    manifest = tmp_path / "Cargo.toml"
    manifest.write_text(toml_content, encoding="utf-8")

    # Record the case metadata for pytest output without affecting behaviour.
    request.node.user_properties.append(("case", case_name))
    request.node.user_properties.append(("description", description))

    original = manifest.read_text(encoding="utf-8")

    publish_workspace_module.prune_workspace_members(manifest)

    content = manifest.read_text(encoding="utf-8")
    if isinstance(expected, list):
        data = tomllib.loads(content)
        assert data["workspace"]["members"] == expected
    else:
        assert content == expected == original
