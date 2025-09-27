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

    run_publish_check_module.run_cargo_command(
        "demo",
        fake_workspace,
        ["cargo", "mock"],
        timeout_secs=5,
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

    run_publish_check_module.run_cargo_command(
        "demo",
        fake_workspace,
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
    fake_local = cargo_test_context.patch_local_runner(
        lambda _args, _timeout: (3, "bad stdout", "bad stderr")
    )

    with (
        cargo_test_context.caplog.at_level("ERROR"),
        pytest.raises(SystemExit) as excinfo,
    ):
        cargo_test_context.run_publish_check_module.run_cargo_command(
            "demo",
            cargo_test_context.fake_workspace,
            ["cargo", "failing"],
            timeout_secs=5,
        )
    assert "exit code 3" in str(excinfo.value)
    assert "bad stdout" in cargo_test_context.caplog.text
    assert "bad stderr" in cargo_test_context.caplog.text
    assert fake_local.cwd_calls == [
        cargo_test_context.fake_workspace / "crates" / "demo"
    ]


def test_run_cargo_command_passes_command_result(
    monkeypatch: pytest.MonkeyPatch,
    patch_local_runner: typ.Callable[[RunCallable], FakeLocal],
    fake_workspace: Path,
    run_publish_check_module: ModuleType,
) -> None:
    """Ensure the failure handler receives the complete command result."""
    fake_local = patch_local_runner(lambda _args, _timeout: (5, "out", "err"))

    observed: dict[str, object] = {}

    def record_failure(crate: str, result: object) -> None:
        observed["crate"] = crate
        observed["result"] = result
        exit_message = "handler invoked"
        raise SystemExit(exit_message)

    monkeypatch.setattr(
        run_publish_check_module, "_handle_command_failure", record_failure
    )

    with pytest.raises(SystemExit, match="handler invoked"):
        run_publish_check_module.run_cargo_command(
            "demo",
            fake_workspace,
            ["cargo", "oops"],
            timeout_secs=9,
        )

    expected = run_publish_check_module.CommandResult(
        command=["cargo", "oops"],
        return_code=5,
        stdout="out",
        stderr="err",
    )
    assert observed == {"crate": "demo", "result": expected}
    assert fake_local.invocations == [(["cargo", "oops"], 9)]


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

    with pytest.raises(SystemExit) as excinfo:
        run_publish_check_module.run_cargo_command(
            "demo",
            fake_workspace,
            ["cargo", "wait"],
            timeout_secs=1,
        )
    assert "timed out" in str(excinfo.value)


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
    mock_cargo_runner: list[tuple[str, Path, list[str], int]],
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

    assert mock_cargo_runner == [(crate, workspace, expected_command, timeout)]
