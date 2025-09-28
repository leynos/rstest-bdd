"""Integration coverage for ``run_publish_check`` dry-run orchestration."""

from __future__ import annotations

import typing as typ

if typ.TYPE_CHECKING:
    from pathlib import Path
    from types import ModuleType

    import pytest


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
