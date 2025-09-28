"""Live publish workflow orchestration tests."""

from __future__ import annotations

import typing as typ

import pytest

if typ.TYPE_CHECKING:
    from pathlib import Path
    from types import ModuleType


def test_process_crates_for_live_publish_delegates_configuration(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    run_publish_check_module: ModuleType,
) -> None:
    """Ensure the live publish wrapper forwards configuration unmodified."""
    captured: dict[str, object] = {}

    def fake_process_crates(
        workspace: Path,
        timeout_secs: int,
        config: object,
        crate_action: object,
    ) -> None:
        captured["workspace"] = workspace
        captured["timeout"] = timeout_secs
        captured["config"] = config
        captured["crate_action"] = crate_action

    monkeypatch.setattr(
        run_publish_check_module, "_process_crates", fake_process_crates
    )

    workspace = tmp_path / "live"
    run_publish_check_module._process_crates_for_live_publish(workspace, 99)

    assert captured["workspace"] == workspace
    assert captured["timeout"] == 99
    config = captured["config"]
    assert isinstance(config, run_publish_check_module.CrateProcessingConfig)
    assert config.strip_patch is False
    assert config.include_local_path is False
    assert config.apply_per_crate is True
    assert config.per_crate_cleanup is run_publish_check_module.remove_patch_entry
    assert captured["crate_action"] is run_publish_check_module.publish_crate_commands


def test_process_crates_for_live_publish_runs_publish_workflow(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    run_publish_check_module: ModuleType,
) -> None:
    """Verify the live publish wrapper applies replacements per crate."""
    workspace = tmp_path / "workspace"
    workspace.mkdir()
    (workspace / "Cargo.toml").write_text("", encoding="utf-8")

    calls: list[tuple[str, object]] = []

    monkeypatch.setattr(
        run_publish_check_module, "strip_patch_section", lambda *_: None
    )
    monkeypatch.setattr(
        run_publish_check_module, "workspace_version", lambda _m: "0.1.0"
    )

    def fake_apply(
        root: Path,
        version: str,
        *,
        include_local_path: bool,
        crates: tuple[str, ...] | None = None,
    ) -> None:
        calls.append(("apply", (root, version, include_local_path, crates)))

    monkeypatch.setattr(
        run_publish_check_module, "apply_workspace_replacements", fake_apply
    )

    def fake_publish(crate: str, root: Path, *, timeout_secs: int) -> None:
        calls.append(("publish", (crate, root, timeout_secs)))

    monkeypatch.setattr(
        run_publish_check_module, "publish_crate_commands", fake_publish
    )

    def fake_remove(manifest: Path, crate: str) -> None:
        calls.append(("remove_patch", (manifest, crate)))

    monkeypatch.setattr(run_publish_check_module, "remove_patch_entry", fake_remove)
    monkeypatch.setattr(
        run_publish_check_module,
        "CRATE_ORDER",
        ("crate-a", "crate-b"),
    )

    run_publish_check_module._process_crates_for_live_publish(workspace, 42)

    manifest = workspace / "Cargo.toml"
    assert calls == [
        ("apply", (workspace, "0.1.0", False, ("crate-a",))),
        ("publish", ("crate-a", workspace, 42)),
        ("remove_patch", (manifest, "crate-a")),
        ("apply", (workspace, "0.1.0", False, ("crate-b",))),
        ("publish", ("crate-b", workspace, 42)),
        ("remove_patch", (manifest, "crate-b")),
    ]


def test_publish_crate_commands_skips_already_published_sequence(
    monkeypatch: pytest.MonkeyPatch,
    caplog: pytest.LogCaptureFixture,
    tmp_path: Path,
    run_publish_check_module: ModuleType,
) -> None:
    """Ensure already-published crates skip the remaining publish commands."""
    workspace = tmp_path / "workspace"
    workspace.mkdir()

    crate = next(iter(run_publish_check_module.LIVE_PUBLISH_COMMANDS))
    commands = run_publish_check_module.LIVE_PUBLISH_COMMANDS[crate]
    observed: dict[str, object] = {}

    def fake_handle_output(stdout: str, stderr: str) -> None:
        observed["streams"] = (stdout, stderr)

    monkeypatch.setattr(
        run_publish_check_module,
        "_handle_command_output",
        fake_handle_output,
    )

    executed: list[tuple[str, ...]] = []

    def fake_run_cargo(
        context: run_publish_check_module.CargoCommandContext,
        command: typ.Sequence[str],
        *,
        on_failure: typ.Callable[[str, run_publish_check_module.CommandResult], bool],
    ) -> None:
        executed.append(tuple(command))
        assert context.crate == crate
        assert context.crate_dir == workspace / "crates" / crate
        assert context.timeout_secs == 123
        if len(executed) == 1:
            result = run_publish_check_module.CommandResult(
                command=command,
                return_code=1,
                stdout="dry run output",
                stderr="error: crate version already exists on crates.io index",
            )
            assert on_failure(context.crate, result) is True
            return

        pytest.fail("publish_crate_commands should stop after handling the failure")

    monkeypatch.setattr(
        run_publish_check_module, "run_cargo_command", fake_run_cargo
    )

    with caplog.at_level("WARNING"):
        run_publish_check_module.publish_crate_commands(
            crate,
            workspace,
            timeout_secs=123,
        )

    assert executed == [tuple(commands[0])]
    assert observed["streams"] == (
        "dry run output",
        "error: crate version already exists on crates.io index",
    )
    assert "already published" in caplog.text


def test_publish_crate_commands_propagates_unhandled_failure(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    run_publish_check_module: ModuleType,
) -> None:
    """Unhandled publish failures must raise to the caller."""
    workspace = tmp_path / "workspace"
    workspace.mkdir()

    crate = next(iter(run_publish_check_module.LIVE_PUBLISH_COMMANDS))
    commands = run_publish_check_module.LIVE_PUBLISH_COMMANDS[crate]
    executed: list[tuple[str, ...]] = []

    def fake_run_cargo(
        context: run_publish_check_module.CargoCommandContext,
        command: typ.Sequence[str],
        *,
        on_failure: typ.Callable[[str, run_publish_check_module.CommandResult], bool],
    ) -> None:
        executed.append(tuple(command))
        result = run_publish_check_module.CommandResult(
            command=command,
            return_code=101,
            stdout="",
            stderr="error: network failure",
        )
        assert context.crate == crate
        assert context.crate_dir == workspace / "crates" / crate
        assert context.timeout_secs == 5
        assert on_failure(context.crate, result) is False
        message = "network failure"
        raise SystemExit(message)

    monkeypatch.setattr(
        run_publish_check_module, "run_cargo_command", fake_run_cargo
    )

    with pytest.raises(SystemExit, match="network failure"):
        run_publish_check_module.publish_crate_commands(
            crate,
            workspace,
            timeout_secs=5,
        )

    assert executed == [tuple(commands[0])]
