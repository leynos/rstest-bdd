"""Command handling behaviour for the publish check workflow."""

from __future__ import annotations

import typing as typ
from pathlib import Path

import pytest

from .conftest import (
    CargoTestContext,
    CommandFailureTestCase,
    FakeLocal,
    RunCallable,
)

if typ.TYPE_CHECKING:
    from types import ModuleType


@pytest.mark.parametrize(
    "test_case",
    [
        CommandFailureTestCase(
            crate="demo",
            result_kwargs={
                "command": ["cargo", "check"],
                "return_code": 7,
                "stdout": "stdout text",
                "stderr": "stderr text",
            },
            expected_exit_fragment="exit code 7",
            expected_logs=("stdout text", "stderr text"),
            unexpected_logs=(),
        ),
        CommandFailureTestCase(
            crate="fmt",
            result_kwargs={
                "command": ["cargo", "fmt"],
                "return_code": 1,
                "stdout": "",
                "stderr": "",
            },
            expected_exit_fragment=None,
            expected_logs=(),
            unexpected_logs=("cargo stdout", "cargo stderr"),
        ),
        CommandFailureTestCase(
            crate="fmt",
            result_kwargs={
                "command": ["cargo", "fmt"],
                "return_code": 5,
                "stdout": b"binary stdout",
                "stderr": b"binary stderr",
            },
            expected_exit_fragment=None,
            expected_logs=("b'binary stdout'", "b'binary stderr'"),
            unexpected_logs=(),
        ),
        CommandFailureTestCase(
            crate="fmt",
            result_kwargs={
                "command": ["cargo", "fmt"],
                "return_code": -9,
                "stdout": "ignored",
                "stderr": "ignored",
            },
            expected_exit_fragment="exit code -9",
            expected_logs=(),
            unexpected_logs=(),
        ),
    ],
    ids=[
        "logs_and_exits",
        "omits_empty_output",
        "accepts_non_string_outputs",
        "reports_negative_exit_codes",
    ],
)
def test_handle_command_failure(
    run_publish_check_module: ModuleType,
    caplog: pytest.LogCaptureFixture,
    test_case: CommandFailureTestCase,
) -> None:
    """Ensure command failures raise ``SystemExit`` and log appropriately."""
    result = run_publish_check_module.CommandResult(**test_case.result_kwargs)

    with caplog.at_level("ERROR"), pytest.raises(SystemExit) as excinfo:
        run_publish_check_module._handle_command_failure(test_case.crate, result)

    if test_case.expected_exit_fragment is not None:
        assert test_case.expected_exit_fragment in str(excinfo.value)

    for text in test_case.expected_logs:
        assert text in caplog.text

    for text in test_case.unexpected_logs:
        assert text not in caplog.text


def test_run_cargo_command_streams_output(
    patch_local_runner: typ.Callable[[RunCallable], FakeLocal],
    fake_workspace: Path,
    capsys: pytest.CaptureFixture[str],
    run_publish_check_module: ModuleType,
) -> None:
    """Verify stdout and stderr from cargo are streamed to the console."""
    crate_dir = fake_workspace / "crates" / "demo"

    fake_local = patch_local_runner(
        lambda _args, _timeout: (0, "cargo ok\n", "cargo warn\n")
    )

    context = run_publish_check_module.build_cargo_command_context(
        "demo",
        fake_workspace,
        timeout_secs=5,
    )
    run_publish_check_module.run_cargo_command(
        context,
        ["cargo", "mock"],
    )

    captured = capsys.readouterr()
    assert "cargo ok" in captured.out
    assert "cargo warn" in captured.err
    assert fake_local.cwd_calls == [crate_dir]
    assert fake_local.env_calls == [{"CARGO_HOME": str(fake_workspace / ".cargo-home")}]
    assert fake_local.invocations == [(["cargo", "mock"], 5)]


def test_run_cargo_command_uses_env_timeout(
    monkeypatch: pytest.MonkeyPatch,
    patch_local_runner: typ.Callable[[RunCallable], FakeLocal],
    fake_workspace: Path,
    run_publish_check_module: ModuleType,
) -> None:
    """Confirm the command respects ``PUBLISH_CHECK_TIMEOUT_SECS`` overrides."""
    crate_dir = fake_workspace / "crates" / "demo"

    fake_local = patch_local_runner(lambda _args, timeout: (0, "", ""))
    monkeypatch.setenv("PUBLISH_CHECK_TIMEOUT_SECS", "11")

    context = run_publish_check_module.build_cargo_command_context(
        "demo",
        fake_workspace,
    )
    run_publish_check_module.run_cargo_command(
        context,
        ["cargo", "mock"],
    )

    assert fake_local.cwd_calls == [crate_dir]
    assert fake_local.env_calls == [{"CARGO_HOME": str(fake_workspace / ".cargo-home")}]
    assert fake_local.invocations == [(["cargo", "mock"], 11)]


def test_run_cargo_command_logs_failures(
    monkeypatch: pytest.MonkeyPatch,
    cargo_test_context: CargoTestContext,
) -> None:
    """Ensure command failures record output and raise ``SystemExit``."""
    module = cargo_test_context.run_publish_check_module
    fake_local = cargo_test_context.patch_local_runner(
        lambda _args, _timeout: (3, "bad stdout", "bad stderr")
    )

    with cargo_test_context.caplog.at_level("ERROR"):
        context = module.build_cargo_command_context(
            "demo",
            cargo_test_context.fake_workspace,
            timeout_secs=5,
        )
        with pytest.raises(SystemExit) as excinfo:
            module.run_cargo_command(
                context,
                ["cargo", "failing"],
            )
    assert "exit code 3" in str(excinfo.value)
    assert "bad stdout" in cargo_test_context.caplog.text
    assert "bad stderr" in cargo_test_context.caplog.text
    assert fake_local.cwd_calls == [
        cargo_test_context.fake_workspace / "crates" / "demo"
    ]


def test_run_cargo_command_suppresses_failure_when_handler_accepts(
    monkeypatch: pytest.MonkeyPatch,
    patch_local_runner: typ.Callable[[RunCallable], FakeLocal],
    fake_workspace: Path,
    run_publish_check_module: ModuleType,
) -> None:
    """Verify ``on_failure`` receives the result and suppresses default handling."""
    fake_local = patch_local_runner(lambda _args, _timeout: (5, "out", "err"))
    observed: dict[str, object] = {}

    def record_failure(
        crate: str,
        result: run_publish_check_module.CommandResult,
    ) -> bool:
        observed["crate"] = crate
        observed["result"] = result
        return True

    monkeypatch.setattr(
        run_publish_check_module,
        "_handle_command_failure",
        lambda *_: pytest.fail("default failure handler should not run"),
    )

    context = run_publish_check_module.build_cargo_command_context(
        "demo",
        fake_workspace,
        timeout_secs=9,
    )
    run_publish_check_module.run_cargo_command(
        context,
        ["cargo", "oops"],
        on_failure=record_failure,
    )

    expected = run_publish_check_module.CommandResult(
        command=["cargo", "oops"],
        return_code=5,
        stdout="out",
        stderr="err",
    )
    assert observed == {"crate": "demo", "result": expected}
    assert fake_local.invocations == [(["cargo", "oops"], 9)]


def test_run_cargo_command_calls_default_when_handler_declines(
    monkeypatch: pytest.MonkeyPatch,
    patch_local_runner: typ.Callable[[RunCallable], FakeLocal],
    fake_workspace: Path,
    run_publish_check_module: ModuleType,
) -> None:
    """Ensure declining handlers allow the default failure logic to run."""
    fake_local = patch_local_runner(lambda _args, _timeout: (17, "out", "err"))
    observed: dict[str, object] = {}

    def decline_failure(
        crate: str,
        result: run_publish_check_module.CommandResult,
    ) -> bool:
        observed["crate"] = crate
        observed["result"] = result
        return False

    def sentinel_failure(
        crate: str,
        result: run_publish_check_module.CommandResult,
    ) -> None:
        message = f"default handler invoked for {crate}"
        raise SystemExit(message)

    monkeypatch.setattr(
        run_publish_check_module,
        "_handle_command_failure",
        sentinel_failure,
    )

    context = run_publish_check_module.build_cargo_command_context(
        "demo",
        fake_workspace,
        timeout_secs=3,
    )
    with pytest.raises(SystemExit, match="default handler invoked"):
        run_publish_check_module.run_cargo_command(
            context,
            ["cargo", "oops"],
            on_failure=decline_failure,
        )

    expected = run_publish_check_module.CommandResult(
        command=["cargo", "oops"],
        return_code=17,
        stdout="out",
        stderr="err",
    )
    assert observed == {"crate": "demo", "result": expected}
    assert fake_local.invocations == [(["cargo", "oops"], 3)]


def test_run_cargo_command_times_out(
    patch_local_runner: typ.Callable[[RunCallable], FakeLocal],
    fake_workspace: Path,
    run_publish_check_module: ModuleType,
) -> None:
    """Raise ``SystemExit`` when cargo times out despite retries."""

    def raise_timeout(_args: list[str], _timeout: int | None) -> tuple[int, str, str]:
        timeout_message = "timeout"
        raise run_publish_check_module.ProcessTimedOut(timeout_message, _args)

    patch_local_runner(raise_timeout)

    context = run_publish_check_module.build_cargo_command_context(
        "demo",
        fake_workspace,
        timeout_secs=1,
    )
    with pytest.raises(SystemExit) as excinfo:
        run_publish_check_module.run_cargo_command(
            context,
            ["cargo", "wait"],
        )
    assert "timed out" in str(excinfo.value)


def test_publish_one_command_handles_already_published(
    monkeypatch: pytest.MonkeyPatch,
    caplog: pytest.LogCaptureFixture,
    tmp_path: Path,
    run_publish_check_module: ModuleType,
) -> None:
    """Ensure already-published failures short-circuit further commands."""
    workspace = tmp_path / "workspace"
    observed_output: dict[str, tuple[str, str]] = {}

    def fake_handle_output(stdout: str, stderr: str) -> None:
        observed_output["streams"] = (stdout, stderr)

    monkeypatch.setattr(
        run_publish_check_module,
        "_handle_command_output",
        fake_handle_output,
    )

    def fake_run_cargo(
        context: run_publish_check_module.CargoCommandContext,
        command: typ.Sequence[str],
        *,
        on_failure: typ.Callable[[str, run_publish_check_module.CommandResult], bool],
    ) -> None:
        result = run_publish_check_module.CommandResult(
            command=command,
            return_code=1,
            stdout="dry run output",
            stderr="error: crate already exists on crates.io index",
        )
        assert context.crate == "demo"
        assert context.crate_dir == workspace / "crates" / "demo"
        assert context.timeout_secs == 11
        assert on_failure(context.crate, result) is True

    monkeypatch.setattr(run_publish_check_module, "run_cargo_command", fake_run_cargo)

    with caplog.at_level("WARNING"):
        handled = run_publish_check_module._publish_one_command(
            "demo",
            workspace,
            ["cargo", "publish"],
            timeout_secs=11,
        )

    assert handled is True
    assert observed_output["streams"] == (
        "dry run output",
        "error: crate already exists on crates.io index",
    )
    assert "already published" in caplog.text


def test_publish_one_command_detects_markers_case_insensitive(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    run_publish_check_module: ModuleType,
) -> None:
    """Ensure already-published markers are matched across streams."""
    workspace = tmp_path / "workspace"

    def fake_run_cargo(
        context: run_publish_check_module.CargoCommandContext,
        command: typ.Sequence[str],
        *,
        on_failure: typ.Callable[[str, run_publish_check_module.CommandResult], bool],
    ) -> None:
        result = run_publish_check_module.CommandResult(
            command=command,
            return_code=1,
            stdout="WARNING: crate VERSION ALREADY EXISTS",  # intentionally upper case
            stderr="",
        )
        assert on_failure(context.crate, result) is True

    monkeypatch.setattr(
        run_publish_check_module,
        "run_cargo_command",
        fake_run_cargo,
    )

    handled = run_publish_check_module._publish_one_command(
        "demo",
        workspace,
        ["cargo", "publish"],
    )

    assert handled is True


@pytest.mark.parametrize(
    ("stdout", "stderr"),
    [
        (
            "warning: aborting upload due to dry run\n",
            "error: crate demo@0.1.0 already exists on crates.io index\n",
        ),
        (
            "",
            (
                "error: api errors (status 200 OK): "
                "crate version 'demo 0.1.0' already uploaded\n"
            ),
        ),
        (
            b"",
            (b"error: crate demo@0.1.0 already exists on crates.io\n"),
        ),
    ],
    ids=["dry_run_warning", "api_error", "bytes_stream"],
)
def test_contains_already_published_marker_handles_known_messages(
    run_publish_check_module: ModuleType,
    stdout: str | bytes,
    stderr: str | bytes,
) -> None:
    """Confirm recognised crates.io markers trigger the already-published path."""
    result = run_publish_check_module.CommandResult(
        command=["cargo", "publish"],
        return_code=101,
        stdout=stdout,
        stderr=stderr,
    )

    assert run_publish_check_module._contains_already_published_marker(result) is True


def test_publish_one_command_returns_false_on_success(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    run_publish_check_module: ModuleType,
) -> None:
    """Confirm successful commands do not request a short-circuit."""
    workspace = tmp_path / "workspace"
    captured: dict[str, object] = {}

    def fake_run_cargo(
        context: run_publish_check_module.CargoCommandContext,
        command: typ.Sequence[str],
        *,
        on_failure: typ.Callable[[str, run_publish_check_module.CommandResult], bool],
    ) -> None:
        captured["crate"] = context.crate
        captured["crate_dir"] = context.crate_dir
        captured["env"] = dict(context.env_overrides)
        captured["command"] = tuple(command)
        captured["timeout"] = context.timeout_secs

    monkeypatch.setattr(run_publish_check_module, "run_cargo_command", fake_run_cargo)

    handled = run_publish_check_module._publish_one_command(
        "demo",
        workspace,
        ["cargo", "publish"],
        timeout_secs=7,
    )

    assert handled is False
    assert captured == {
        "crate": "demo",
        "crate_dir": workspace / "crates" / "demo",
        "env": {"CARGO_HOME": str(workspace / ".cargo-home")},
        "command": ("cargo", "publish"),
        "timeout": 7,
    }


def test_publish_one_command_propagates_unhandled_errors(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    run_publish_check_module: ModuleType,
) -> None:
    """Unhandled failures must bubble out for the caller to process."""
    workspace = tmp_path / "workspace"

    def fake_run_cargo(
        context: run_publish_check_module.CargoCommandContext,
        command: typ.Sequence[str],
        *,
        on_failure: typ.Callable[[str, run_publish_check_module.CommandResult], bool],
    ) -> None:
        result = run_publish_check_module.CommandResult(
            command=command,
            return_code=2,
            stdout="",  # stdout intentionally empty
            stderr="error: publishing failed",
        )
        assert context.crate == "demo"
        assert context.crate_dir == workspace / "crates" / "demo"
        assert on_failure(context.crate, result) is False
        message = "unhandled failure"
        raise SystemExit(message)

    monkeypatch.setattr(run_publish_check_module, "run_cargo_command", fake_run_cargo)

    with pytest.raises(SystemExit, match="unhandled failure"):
        run_publish_check_module._publish_one_command(
            "demo",
            workspace,
            ["cargo", "publish"],
        )


@pytest.mark.parametrize(
    ("function_and_command", "test_scenario"),
    [
        (
            ("package_crate", ["cargo", "package", "--allow-dirty", "--no-verify"]),
            ("demo", 42),
        ),
        (
            ("check_crate", ["cargo", "check", "--all-features"]),
            ("demo", 17),
        ),
    ],
    ids=["package_crate_invocation", "check_crate_invocation"],
)
def test_cargo_commands_invoke_runner(
    run_publish_check_module: ModuleType,
    mock_cargo_runner: list[
        tuple[object, list[str], typ.Callable[[str, object], bool] | None]
    ],
    function_and_command: tuple[str, list[str]],
    test_scenario: tuple[str, int],
) -> None:
    """Ensure cargo helper wrappers delegate to ``run_cargo_command``."""
    function_name, expected_command = function_and_command
    crate, timeout = test_scenario
    workspace = Path("/safe/workspace")

    getattr(run_publish_check_module, function_name)(
        crate, workspace, timeout_secs=timeout
    )

    assert len(mock_cargo_runner) == 1
    context, observed_command, on_failure = mock_cargo_runner[0]
    assert context.crate == crate
    assert context.crate_dir == workspace / "crates" / crate
    assert context.env_overrides == {"CARGO_HOME": str(workspace / ".cargo-home")}
    assert context.timeout_secs == timeout
    assert observed_command == expected_command
    assert on_failure is None
