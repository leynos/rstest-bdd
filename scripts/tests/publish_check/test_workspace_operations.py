"""Workspace export and pruning behaviour for publish checks."""

from __future__ import annotations

import contextlib
from pathlib import Path
from types import ModuleType

import pytest


def test_export_workspace_creates_manifest_copy(
    run_publish_check_module: ModuleType, tmp_path: Path
) -> None:
    destination = tmp_path / "workspace"
    destination.mkdir()

    run_publish_check_module.export_workspace(destination)

    assert (destination / "Cargo.toml").exists()


def test_export_workspace_propagates_git_failure(
    monkeypatch: pytest.MonkeyPatch,
    publish_workspace_module: ModuleType,
    tmp_path: Path,
) -> None:
    class FakeCommand:
        def __getitem__(self, _args: object) -> "FakeCommand":
            return self

        def __call__(self, *_args: object, **_kwargs: object) -> None:
            raise RuntimeError("archive failed")

    class FakeLocal:
        def __getitem__(self, name: str) -> FakeCommand:
            assert name == "git"
            return FakeCommand()

        @contextlib.contextmanager
        def cwd(self, _path: Path):
            yield

    monkeypatch.setattr(publish_workspace_module, "local", FakeLocal())

    with pytest.raises(RuntimeError, match="archive failed"):
        publish_workspace_module.export_workspace(tmp_path)
