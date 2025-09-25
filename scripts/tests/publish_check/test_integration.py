"""Integration coverage for ``run_publish_check`` orchestration."""

from __future__ import annotations

from pathlib import Path
from types import ModuleType
from typing import Callable

import pytest


def test_run_publish_check_orchestrates_workflow(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    run_publish_check_module: ModuleType,
) -> None:
    workspace_dir = tmp_path / "workspace"
    workspace_dir.mkdir()
    monkeypatch.setattr(
        run_publish_check_module.tempfile, "mkdtemp", lambda: str(workspace_dir)
    )

    steps: list[tuple[str, object]] = []

    def record(step: str) -> Callable[..., None]:
        def _inner(*args: object, **_kwargs: object) -> None:
            steps.append((step, args[0]))

        return _inner

    def fake_workspace_version(_manifest: Path) -> str:
        steps.append(("version", _manifest))
        return "9.9.9"

    package_calls: list[tuple[str, Path, int]] = []
    check_calls: list[tuple[str, Path, int]] = []

    def fake_package(crate: str, root: Path, *, timeout_secs: int) -> None:
        package_calls.append((crate, root, timeout_secs))

    def fake_check(crate: str, root: Path, *, timeout_secs: int) -> None:
        check_calls.append((crate, root, timeout_secs))

    monkeypatch.setattr(run_publish_check_module, "export_workspace", record("export"))
    monkeypatch.setattr(
        run_publish_check_module, "prune_workspace_members", record("prune")
    )
    monkeypatch.setattr(
        run_publish_check_module, "strip_patch_section", record("strip")
    )
    monkeypatch.setattr(
        run_publish_check_module,
        "workspace_version",
        fake_workspace_version,
    )

    def fake_apply(
        root: Path,
        version: str,
        *,
        include_local_path: bool,
        crates: tuple[str, ...] | None = None,
    ) -> None:
        steps.append(("apply", (root, version, include_local_path, crates)))

    monkeypatch.setattr(
        run_publish_check_module,
        "apply_workspace_replacements",
        fake_apply,
    )
    monkeypatch.setattr(
        run_publish_check_module,
        "remove_patch_entry",
        lambda *_args, **_kwargs: None,
    )
    monkeypatch.setattr(run_publish_check_module, "package_crate", fake_package)
    monkeypatch.setattr(run_publish_check_module, "check_crate", fake_check)
    monkeypatch.setattr(
        run_publish_check_module,
        "CRATE_ORDER",
        ("rstest-bdd-patterns", "demo-crate"),
    )

    run_publish_check_module.run_publish_check(keep_tmp=False, timeout_secs=15)

    manifest_path = workspace_dir / "Cargo.toml"
    assert steps[:3] == [
        ("export", workspace_dir),
        ("prune", manifest_path),
        ("strip", manifest_path),
    ]
    assert ("version", manifest_path) in steps
    assert ("apply", (workspace_dir, "9.9.9", True, None)) in steps
    assert package_calls == [("rstest-bdd-patterns", workspace_dir, 15)]
    assert check_calls == [("demo-crate", workspace_dir, 15)]
    assert not workspace_dir.exists()


class TestRunPublishCheckLiveMode:
    """Behavioural checks for live publish orchestration."""

    def _setup_test_workspace(
        self,
        tmp_path: Path,
        monkeypatch: pytest.MonkeyPatch,
        run_publish_check_module: ModuleType,
    ) -> tuple[Path, Path]:
        """Prepare a temporary workspace and redirect ``mkdtemp``.

        Parameters
        ----------
        tmp_path:
            Base temporary directory provided by pytest.
        monkeypatch:
            Fixture used to redirect ``mkdtemp`` to the prepared workspace.
        run_publish_check_module:
            Module under test whose ``tempfile`` helper is patched.

        Returns
        -------
        tuple[Path, Path]
            The workspace directory and its manifest path.
        """

        workspace_dir = tmp_path / "live"
        workspace_dir.mkdir()
        manifest = workspace_dir / "Cargo.toml"
        manifest.write_text(
            "[workspace]\n"
            "[patch.crates-io]\n"
            "demo-crate = { path = \"crates/demo-crate\" }\n",
            encoding="utf-8",
        )
        monkeypatch.setattr(
            run_publish_check_module.tempfile, "mkdtemp", lambda: str(workspace_dir)
        )
        return workspace_dir, manifest

    def _setup_mocking_and_recording(
        self,
        monkeypatch: pytest.MonkeyPatch,
        run_publish_check_module: ModuleType,
    ) -> tuple[list[tuple[str, object]], list[tuple[str, Path, list[str], int]]]:
        """Register spies for workspace operations and cargo invocations.

        Parameters
        ----------
        monkeypatch:
            Fixture that rewires functions on the module under test.
        run_publish_check_module:
            Module supplying publish helpers to replace with spies.

        Returns
        -------
        tuple[list[tuple[str, object]], list[tuple[str, Path, list[str], int]]]
            Recorded workspace steps and cargo invocations.
        """

        steps: list[tuple[str, object]] = []

        def record(step: str) -> Callable[[Path], None]:
            def _inner(target: Path) -> None:
                steps.append((step, target))

            return _inner

        def fake_apply(
            root: Path,
            version: str,
            *,
            include_local_path: bool,
            crates: tuple[str, ...] | None = None,
        ) -> None:
            steps.append(("apply", (root, version, include_local_path, crates)))

        monkeypatch.setattr(
            run_publish_check_module, "export_workspace", record("export")
        )
        monkeypatch.setattr(
            run_publish_check_module, "prune_workspace_members", record("prune")
        )
        monkeypatch.setattr(
            run_publish_check_module, "strip_patch_section", record("strip")
        )
        monkeypatch.setattr(
            run_publish_check_module, "workspace_version", lambda _manifest: "1.2.3"
        )
        monkeypatch.setattr(
            run_publish_check_module,
            "apply_workspace_replacements",
            fake_apply,
        )

        def fake_remove(manifest_path: Path, crate: str) -> None:
            steps.append(("remove_patch", (manifest_path, crate)))

        monkeypatch.setattr(run_publish_check_module, "remove_patch_entry", fake_remove)

        commands: list[tuple[str, Path, list[str], int]] = []

        def fake_run_cargo(
            crate: str,
            workspace_root: Path,
            command: list[str],
            *,
            timeout_secs: int,
        ) -> None:
            commands.append((crate, workspace_root, command, timeout_secs))

        monkeypatch.setattr(run_publish_check_module, "run_cargo_command", fake_run_cargo)
        monkeypatch.setattr(
            run_publish_check_module,
            "CRATE_ORDER",
            ("demo-crate",),
        )
        monkeypatch.setattr(
            run_publish_check_module,
            "LIVE_PUBLISH_COMMANDS",
            {"demo-crate": (("cargo", "publish", "--dry-run"), ("cargo", "publish"))},
        )

        return steps, commands

    def _verify_live_publish_execution(
        self,
        steps: list[tuple[str, object]],
        commands: list[tuple[str, Path, list[str], int]],
        workspace_dir: Path,
        manifest: Path,
    ) -> None:
        """Assert the live workflow executed the expected steps.

        Parameters
        ----------
        steps:
            Recorded workspace operations.
        commands:
            Captured cargo invocations.
        workspace_dir:
            Workspace directory that should be removed after execution.
        manifest:
            Workspace manifest used for assertions.
        """

        assert steps[:2] == [
            ("export", workspace_dir),
            ("prune", manifest),
        ]
        assert ("strip", manifest) not in steps
        assert ("remove_patch", (manifest, "demo-crate")) in steps
        assert ("apply", (workspace_dir, "1.2.3", False, ("demo-crate",))) in steps
        assert commands == [
            ("demo-crate", workspace_dir, ["cargo", "publish", "--dry-run"], 30),
            ("demo-crate", workspace_dir, ["cargo", "publish"], 30),
        ]
        assert not workspace_dir.exists()

    def test_run_publish_check_live_mode_invokes_publish_commands(
        self,
        monkeypatch: pytest.MonkeyPatch,
        tmp_path: Path,
        run_publish_check_module: ModuleType,
    ) -> None:
        """Test that live mode executes the correct publish commands."""

        workspace_dir, manifest = self._setup_test_workspace(
            tmp_path, monkeypatch, run_publish_check_module
        )
        steps, commands = self._setup_mocking_and_recording(
            monkeypatch, run_publish_check_module
        )

        run_publish_check_module.run_publish_check(
            keep_tmp=False,
            timeout_secs=30,
            live=True,
        )

        self._verify_live_publish_execution(steps, commands, workspace_dir, manifest)


def test_run_publish_check_preserves_workspace(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
    run_publish_check_module: ModuleType,
) -> None:
    workspace_dir = tmp_path / "persist"
    workspace_dir.mkdir()
    monkeypatch.setattr(
        run_publish_check_module.tempfile, "mkdtemp", lambda: str(workspace_dir)
    )
    monkeypatch.setattr(run_publish_check_module, "export_workspace", lambda _dest: None)
    monkeypatch.setattr(run_publish_check_module, "prune_workspace_members", lambda _m: None)
    monkeypatch.setattr(run_publish_check_module, "strip_patch_section", lambda _m: None)
    monkeypatch.setattr(run_publish_check_module, "workspace_version", lambda _m: "1.0.0")
    monkeypatch.setattr(
        run_publish_check_module, "apply_workspace_replacements", lambda *_args, **_kwargs: None
    )
    monkeypatch.setattr(run_publish_check_module, "package_crate", lambda *_args, **_kwargs: None)
    monkeypatch.setattr(run_publish_check_module, "check_crate", lambda *_args, **_kwargs: None)
    monkeypatch.setattr(run_publish_check_module, "CRATE_ORDER", ())

    run_publish_check_module.run_publish_check(keep_tmp=True, timeout_secs=5)

    captured = capsys.readouterr()
    assert "preserving workspace" in captured.out
    assert workspace_dir.exists()


def test_run_publish_check_rejects_non_positive_timeout(
    run_publish_check_module: ModuleType,
) -> None:
    with pytest.raises(SystemExit, match="positive integer"):
        run_publish_check_module.run_publish_check(keep_tmp=False, timeout_secs=0)
