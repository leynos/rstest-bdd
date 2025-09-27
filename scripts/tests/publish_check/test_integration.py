"""Integration coverage for ``run_publish_check`` orchestration."""

from __future__ import annotations

import typing as typ
from dataclasses import dataclass  # noqa: ICN003 - required for WorkflowTestConfig

import pytest

if typ.TYPE_CHECKING:
    from pathlib import Path
    from types import ModuleType


@dataclass
class WorkspaceMocks:
    """Bundle of mock functions for workspace operations."""

    record: typ.Callable[[str], typ.Callable[[Path], None]]
    fake_apply: typ.Callable[..., None]
    fake_remove: typ.Callable[[Path, str], None]


@dataclass
class WorkflowTestConfig:
    """Configuration bundle for workflow integration scaffolding."""

    workspace_name: str
    crate_order: tuple[str, ...] = ("demo-crate",)


class TestRunPublishCheckOrchestration:
    """Integration coverage of the dry-run orchestration workflow."""

    def _create_test_workspace(
        self,
        monkeypatch: pytest.MonkeyPatch,
        tmp_path: Path,
        run_publish_check_module: ModuleType,
    ) -> Path:
        """Create workspace directory and configure ``tempfile`` redirection."""
        workspace_dir = tmp_path / "workspace"
        workspace_dir.mkdir()
        monkeypatch.setattr(
            run_publish_check_module.tempfile, "mkdtemp", lambda: str(workspace_dir)
        )
        return workspace_dir

    def _create_mocks_and_captures(
        self,
    ) -> tuple[
        list[tuple[str, object]],
        list[tuple[str, Path, int]],
        list[tuple[str, Path, int]],
        dict[str, typ.Callable[..., object]],
    ]:
        """Create capture containers and reusable mock functions."""
        steps: list[tuple[str, object]] = []

        def record(step: str) -> typ.Callable[..., None]:
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

        return (
            steps,
            package_calls,
            check_calls,
            {
                "record": record,
                "fake_workspace_version": fake_workspace_version,
                "fake_package": fake_package,
                "fake_check": fake_check,
            },
        )

    def _apply_workflow_patches(
        self,
        monkeypatch: pytest.MonkeyPatch,
        run_publish_check_module: ModuleType,
        mock_functions: dict[str, typ.Callable[..., object]],
        steps: list[tuple[str, object]],
    ) -> None:
        """Apply monkeypatch operations to replace workflow helpers."""
        record = mock_functions["record"]
        fake_workspace_version = mock_functions["fake_workspace_version"]
        fake_package = mock_functions["fake_package"]
        fake_check = mock_functions["fake_check"]

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

    def _setup_workflow_mocks(
        self,
        monkeypatch: pytest.MonkeyPatch,
        tmp_path: Path,
        run_publish_check_module: ModuleType,
    ) -> tuple[
        Path,
        list[tuple[str, object]],
        list[tuple[str, Path, int]],
        list[tuple[str, Path, int]],
    ]:
        """Set up workspace, mocks, and capture containers for orchestration tests.

        Parameters
        ----------
        monkeypatch:
            Fixture used to rewire helpers on the module under test.
        tmp_path:
            Base temporary directory provided by pytest for isolation.
        run_publish_check_module:
            Imported module exposing the workflow entrypoint and dependencies.

        Returns
        -------
        tuple
            The prepared workspace directory and the lists capturing recorded
            workspace steps, package invocations, and check invocations.
        """
        workspace_dir = self._create_test_workspace(
            monkeypatch, tmp_path, run_publish_check_module
        )
        (
            steps,
            package_calls,
            check_calls,
            mock_functions,
        ) = self._create_mocks_and_captures()
        self._apply_workflow_patches(
            monkeypatch, run_publish_check_module, mock_functions, steps
        )
        return workspace_dir, steps, package_calls, check_calls

    def _execute_workflow(self, run_publish_check_module: ModuleType) -> None:
        """Execute ``run_publish_check`` with the standard dry-run arguments.

        Parameters
        ----------
        run_publish_check_module:
            Module under test providing the ``run_publish_check`` entrypoint.
        """
        run_publish_check_module.run_publish_check(keep_tmp=False, timeout_secs=15)

    def _verify_workflow_execution(
        self,
        workspace_dir: Path,
        steps: list[tuple[str, object]],
        package_calls: list[tuple[str, Path, int]],
        check_calls: list[tuple[str, Path, int]],
    ) -> None:
        """Verify the recorded operations and workspace cleanup for the dry run.

        Parameters
        ----------
        workspace_dir:
            Workspace directory created for the test run.
        steps:
            Recorded workspace-level operations.
        package_calls:
            Captured invocations to ``package_crate``.
        check_calls:
            Captured invocations to ``check_crate``.
        """
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

    def test_run_publish_check_orchestrates_workflow(
        self,
        monkeypatch: pytest.MonkeyPatch,
        tmp_path: Path,
        run_publish_check_module: ModuleType,
    ) -> None:
        """Test that ``run_publish_check`` orchestrates the dry workflow correctly."""
        workspace_dir, steps, package_calls, check_calls = self._setup_workflow_mocks(
            monkeypatch, tmp_path, run_publish_check_module
        )

        self._execute_workflow(run_publish_check_module)

        self._verify_workflow_execution(
            workspace_dir, steps, package_calls, check_calls
        )


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
            'demo-crate = { path = "crates/demo-crate" }\n',
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
        steps, record = self._setup_recording_infrastructure()
        fake_apply, fake_remove = self._create_fake_functions(steps)
        commands, fake_run_cargo = self._setup_cargo_recording()
        self._workspace_mocks = WorkspaceMocks(
            record=record,
            fake_apply=fake_apply,
            fake_remove=fake_remove,
        )
        self._setup_workspace_mocks(monkeypatch, run_publish_check_module)
        self._setup_cargo_and_config_mocks(
            monkeypatch,
            run_publish_check_module,
            fake_run_cargo,
        )

        return steps, commands

    def _setup_recording_infrastructure(
        self,
    ) -> tuple[
        list[tuple[str, object]],
        typ.Callable[[str], typ.Callable[[Path], None]],
    ]:
        """Provide shared recording helpers for workspace operations."""
        steps: list[tuple[str, object]] = []

        def record(step: str) -> typ.Callable[[Path], None]:
            def _inner(target: Path) -> None:
                steps.append((step, target))

            return _inner

        return steps, record

    def _create_fake_functions(
        self, steps: list[tuple[str, object]]
    ) -> tuple[typ.Callable[..., None], typ.Callable[[Path, str], None]]:
        """Generate workspace helpers that append their inputs to ``steps``."""

        def fake_apply(
            root: Path,
            version: str,
            *,
            include_local_path: bool,
            crates: tuple[str, ...] | None = None,
        ) -> None:
            steps.append(("apply", (root, version, include_local_path, crates)))

        def fake_remove(manifest_path: Path, crate: str) -> None:
            steps.append(("remove_patch", (manifest_path, crate)))

        return fake_apply, fake_remove

    def _setup_cargo_recording(
        self,
    ) -> tuple[list[tuple[str, Path, list[str], int]], typ.Callable[..., None]]:
        """Capture cargo invocations while preserving their call signature."""
        commands: list[tuple[str, Path, list[str], int]] = []

        def fake_run_cargo(
            crate: str,
            workspace_root: Path,
            command: typ.Sequence[str],
            *,
            timeout_secs: int,
            **kwargs: typ.Any,
        ) -> None:
            unexpected = set(kwargs) - {"on_failure"}
            if unexpected:
                pytest.fail(f"unexpected kwargs passed to fake_run_cargo: {unexpected}")
            commands.append((crate, workspace_root, list(command), timeout_secs))

        return commands, fake_run_cargo

    def _setup_workspace_mocks(
        self,
        monkeypatch: pytest.MonkeyPatch,
        run_publish_check_module: ModuleType,
    ) -> None:
        """Replace workspace helpers with recording doubles."""
        monkeypatch.setattr(
            run_publish_check_module,
            "export_workspace",
            self._workspace_mocks.record("export"),
        )
        monkeypatch.setattr(
            run_publish_check_module,
            "prune_workspace_members",
            self._workspace_mocks.record("prune"),
        )
        monkeypatch.setattr(
            run_publish_check_module,
            "strip_patch_section",
            self._workspace_mocks.record("strip"),
        )
        monkeypatch.setattr(
            run_publish_check_module, "workspace_version", lambda _manifest: "1.2.3"
        )
        monkeypatch.setattr(
            run_publish_check_module,
            "apply_workspace_replacements",
            self._workspace_mocks.fake_apply,
        )
        monkeypatch.setattr(
            run_publish_check_module,
            "remove_patch_entry",
            self._workspace_mocks.fake_remove,
        )

    def _setup_cargo_and_config_mocks(
        self,
        monkeypatch: pytest.MonkeyPatch,
        run_publish_check_module: ModuleType,
        fake_run_cargo: typ.Callable[..., None],
    ) -> None:
        """Configure cargo helpers and static data for the live flow."""
        monkeypatch.setattr(
            run_publish_check_module, "run_cargo_command", fake_run_cargo
        )
        monkeypatch.setattr(
            run_publish_check_module,
            "CRATE_ORDER",
            ("demo-crate",),
        )
        monkeypatch.setattr(
            run_publish_check_module,
            "LIVE_PUBLISH_COMMANDS",
            {
                "demo-crate": (
                    ("cargo", "publish", "--dry-run"),
                    ("cargo", "publish"),
                )
            },
        )

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


def _setup_basic_workflow_mocks(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    run_publish_check_module: ModuleType,
    *,
    config: WorkflowTestConfig,
) -> Path:
    """Prepare shared workspace and helper mocks for workflow integration tests."""
    workspace_dir = tmp_path / config.workspace_name
    workspace_dir.mkdir()
    monkeypatch.setattr(
        run_publish_check_module.tempfile, "mkdtemp", lambda: str(workspace_dir)
    )
    monkeypatch.setattr(
        run_publish_check_module, "export_workspace", lambda _dest: None
    )
    monkeypatch.setattr(
        run_publish_check_module, "prune_workspace_members", lambda _manifest: None
    )
    monkeypatch.setattr(
        run_publish_check_module, "strip_patch_section", lambda _manifest: None
    )
    monkeypatch.setattr(
        run_publish_check_module, "workspace_version", lambda _manifest: "1.0.0"
    )
    monkeypatch.setattr(
        run_publish_check_module,
        "apply_workspace_replacements",
        lambda *_args, **_kwargs: None,
    )
    monkeypatch.setattr(
        run_publish_check_module, "package_crate", lambda *_args, **_kwargs: None
    )
    monkeypatch.setattr(
        run_publish_check_module, "check_crate", lambda *_args, **_kwargs: None
    )
    monkeypatch.setattr(run_publish_check_module, "CRATE_ORDER", config.crate_order)
    return workspace_dir


def test_run_publish_check_preserves_workspace(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
    run_publish_check_module: ModuleType,
) -> None:
    """Ensure the workflow keeps the workspace when requested."""
    workspace_dir = _setup_basic_workflow_mocks(
        monkeypatch,
        tmp_path,
        run_publish_check_module,
        config=WorkflowTestConfig(workspace_name="persist"),
    )

    run_publish_check_module.run_publish_check(keep_tmp=True, timeout_secs=5)

    captured = capsys.readouterr()
    assert "preserving workspace" in captured.out
    assert workspace_dir.exists()


def test_run_publish_check_errors_when_crate_order_empty(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    run_publish_check_module: ModuleType,
) -> None:
    """Verify the workflow aborts if ``CRATE_ORDER`` is empty."""
    _setup_basic_workflow_mocks(
        monkeypatch,
        tmp_path,
        run_publish_check_module,
        config=WorkflowTestConfig(
            workspace_name="missing-order",
            crate_order=(),
        ),
    )

    with pytest.raises(SystemExit, match="CRATE_ORDER must not be empty"):
        run_publish_check_module.run_publish_check(keep_tmp=False, timeout_secs=5)


def test_run_publish_check_rejects_non_positive_timeout(
    run_publish_check_module: ModuleType,
) -> None:
    """Reject configurations that specify a timeout below one second."""
    with pytest.raises(SystemExit, match="positive integer"):
        run_publish_check_module.run_publish_check(keep_tmp=False, timeout_secs=0)
