"""Behavioural coverage for live publish workflow orchestration."""

from __future__ import annotations

import typing as typ

import pytest

from .conftest import WorkspaceMocks

if typ.TYPE_CHECKING:
    from pathlib import Path
    from types import ModuleType


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
    ) -> tuple[
        list[tuple[str, object]],
        list[tuple[object, list[str], typ.Callable[[str, object], bool] | None]],
    ]:
        """Register spies for workspace operations and cargo invocations.

        Parameters
        ----------
        monkeypatch:
            Fixture that rewires functions on the module under test.
        run_publish_check_module:
            Module supplying publish helpers to replace with spies.

        Returns
        -------
        tuple
            Recorded workspace steps and cargo invocations, including the
            resolved cargo contexts and optional failure callbacks.
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
    ) -> tuple[
        list[tuple[object, list[str], typ.Callable[[str, object], bool] | None]],
        typ.Callable[..., None],
    ]:
        """Capture cargo invocations while preserving their call signature."""
        commands: list[
            tuple[object, list[str], typ.Callable[[str, object], bool] | None]
        ] = []

        def fake_run_cargo(
            context: object,
            command: typ.Sequence[str],
            *,
            on_failure: typ.Callable[[str, object], bool] | None = None,
            **kwargs: object,
        ) -> None:
            if kwargs:
                pytest.fail(
                    f"unexpected kwargs passed to fake_run_cargo: {set(kwargs)}"
                )
            commands.append((context, list(command), on_failure))

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
        commands: list[
            tuple[object, list[str], typ.Callable[[str, object], bool] | None]
        ],
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
        assert len(commands) == 2
        for context_obj, _command_args, on_failure in commands:
            assert context_obj.crate == "demo-crate"
            assert context_obj.crate_dir == workspace_dir / "crates" / "demo-crate"
            assert context_obj.env_overrides == {
                "CARGO_HOME": str(workspace_dir / ".cargo-home")
            }
            assert context_obj.timeout_secs == 30
            assert on_failure is not None
        assert commands[0][1] == ["cargo", "publish", "--dry-run"]
        assert commands[1][1] == ["cargo", "publish"]
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
