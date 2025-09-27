"""Workspace export workflow tests."""

from __future__ import annotations

import contextlib
import typing as typ

import pytest

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

        def __call__(self, *_args: object, **_kwargs: object) -> None:
            error_message = "archive failed"
            raise RuntimeError(error_message)

    class FakeLocal:
        def __getitem__(self, name: str) -> FakeCommand:
            if name != "git":
                msg = f"FakeLocal expected 'git' command but received {name!r}"
                raise RuntimeError(msg)
            return FakeCommand()

        def cwd(self, _path: Path) -> contextlib.AbstractContextManager[None]:
            return contextlib.nullcontext()

    monkeypatch.setattr(publish_workspace_module, "local", FakeLocal())

    with pytest.raises(RuntimeError, match="archive failed"):
        publish_workspace_module.export_workspace(tmp_path)
