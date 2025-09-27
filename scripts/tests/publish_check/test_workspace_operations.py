"""Workspace export workflow tests."""

from __future__ import annotations

import contextlib
import typing as typ

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


def test_prune_workspace_members_removes_non_crate_entries(
    publish_workspace_module: ModuleType, tmp_path: Path
) -> None:
    """Remove workspace members that are not part of the publishable crates."""
    manifest = tmp_path / "Cargo.toml"
    manifest.write_text(
        "\n".join(
            (
                "[workspace]",
                "members = [",
                '    "crates/rstest-bdd",',
                '    "examples/todo-cli",',
                "]",
            )
        ),
        encoding="utf-8",
    )

    publish_workspace_module.prune_workspace_members(manifest)

    data = tomllib.loads(manifest.read_text(encoding="utf-8"))
    assert data["workspace"]["members"] == ["crates/rstest-bdd"]


def test_prune_workspace_members_keeps_known_crate_names(
    publish_workspace_module: ModuleType, tmp_path: Path
) -> None:
    """Retain crate entries even when they use alternate directory layouts."""
    manifest = tmp_path / "Cargo.toml"
    manifest.write_text(
        "\n".join(
            (
                "[workspace]",
                'members = ["packages/rstest-bdd", "tools/xtask"]',
            )
        ),
        encoding="utf-8",
    )

    publish_workspace_module.prune_workspace_members(manifest)

    data = tomllib.loads(manifest.read_text(encoding="utf-8"))
    assert data["workspace"]["members"] == ["packages/rstest-bdd"]
