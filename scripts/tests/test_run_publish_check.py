"""Validate scripts.run_publish_check end-to-end.

The suite covers cargo invocation handling, timeout propagation, error reporting, and the temporary workspace export and pruning steps performed before packaging so the publish check remains safe. Tests are expected to run under pytest with local fakes, ensuring release automation can be exercised without invoking real tooling.
"""

from __future__ import annotations

import contextlib
import importlib.util
import sys
from pathlib import Path
from typing import Callable

import pytest

SCRIPTS_DIR = Path(__file__).resolve().parents[1]
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))


@pytest.fixture(scope="module")
def run_publish_check_module():
    spec = importlib.util.spec_from_file_location(
        "run_publish_check", SCRIPTS_DIR / "run_publish_check.py"
    )
    module = importlib.util.module_from_spec(spec)
    assert spec and spec.loader
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class FakeCargoInvocation:
    def __init__(self, local: "FakeLocal", args: list[str]):
        self._local = local
        self._args = ["cargo", *args]

    def run(self, *, retcode: object | None, timeout: int | None) -> tuple[int, str, str]:
        self._local.invocations.append((self._args, timeout))
        return self._local.run_callable(self._args, timeout)


class FakeCargo:
    def __init__(self, local: "FakeLocal") -> None:
        self._local = local

    def __getitem__(self, args: object) -> FakeCargoInvocation:
        if isinstance(args, (list, tuple)):
            extras = list(args)
        else:
            extras = [str(args)]
        return FakeCargoInvocation(self._local, extras)


class FakeLocal:
    def __init__(self, run_callable: Callable[[list[str], int | None], tuple[int, str, str]]):
        self.run_callable = run_callable
        self.cwd_calls: list[Path] = []
        self.env_calls: list[dict[str, str]] = []
        self.invocations: list[tuple[list[str], int | None]] = []

    def __getitem__(self, command: str) -> FakeCargo:
        assert command == "cargo"
        return FakeCargo(self)

    def cwd(self, path: Path):
        self.cwd_calls.append(path)
        return contextlib.nullcontext()

    def env(self, **kwargs: str):
        self.env_calls.append(kwargs)
        return contextlib.nullcontext()


def test_export_workspace_creates_manifest_copy(
    run_publish_check_module, tmp_path: Path
) -> None:
    destination = tmp_path / "workspace"
    destination.mkdir()

    run_publish_check_module.export_workspace(destination)

    assert (destination / "Cargo.toml").exists()


def test_export_workspace_propagates_git_failure(
    monkeypatch: pytest.MonkeyPatch, run_publish_check_module, tmp_path: Path
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

    monkeypatch.setattr(run_publish_check_module, "local", FakeLocal())

    with pytest.raises(RuntimeError, match="archive failed"):
        run_publish_check_module.export_workspace(tmp_path)


def test_handle_command_failure_logs_and_exits(
    run_publish_check_module, caplog: pytest.LogCaptureFixture
) -> None:
    result = run_publish_check_module.CommandResult(
        command=["cargo", "check"],
        return_code=7,
        stdout="stdout text",
        stderr="stderr text",
    )

    with caplog.at_level("ERROR"):
        with pytest.raises(SystemExit) as excinfo:
            run_publish_check_module._handle_command_failure("demo", result)
    message = str(excinfo.value)
    assert "exit code 7" in message
    assert "stdout text" in caplog.text
    assert "stderr text" in caplog.text


def test_handle_command_failure_supports_legacy_signature(
    run_publish_check_module, caplog: pytest.LogCaptureFixture
) -> None:
    with caplog.at_level("ERROR"):
        with pytest.raises(SystemExit) as excinfo:
            run_publish_check_module._handle_command_failure(
                "demo",
                ["cargo", "check"],
                7,
                "stdout text",
                "stderr text",
            )
    message = str(excinfo.value)
    assert "exit code 7" in message
    assert "stdout text" in caplog.text
    assert "stderr text" in caplog.text


def test_run_cargo_command_streams_output(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
    run_publish_check_module,
) -> None:
    workspace = tmp_path / "workspace"
    crate_dir = workspace / "crates" / "demo"
    crate_dir.mkdir(parents=True)

    fake_local = FakeLocal(lambda _args, _timeout: (0, "cargo ok\n", "cargo warn\n"))
    monkeypatch.setattr(run_publish_check_module, "local", fake_local)

    run_publish_check_module.run_cargo_command(
        "demo",
        workspace,
        ["cargo", "mock"],
        timeout_secs=5,
    )

    captured = capsys.readouterr()
    assert "cargo ok" in captured.out
    assert "cargo warn" in captured.err
    assert fake_local.cwd_calls == [crate_dir]
    assert fake_local.env_calls == [
        {"CARGO_HOME": str(workspace / ".cargo-home")}
    ]
    assert fake_local.invocations == [(["cargo", "mock"], 5)]


def test_run_cargo_command_uses_env_timeout(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    run_publish_check_module,
) -> None:
    workspace = tmp_path / "workspace"
    crate_dir = workspace / "crates" / "demo"
    crate_dir.mkdir(parents=True)

    fake_local = FakeLocal(lambda _args, timeout: (0, "", ""))
    monkeypatch.setattr(run_publish_check_module, "local", fake_local)
    monkeypatch.setenv("PUBLISH_CHECK_TIMEOUT_SECS", "11")

    run_publish_check_module.run_cargo_command(
        "demo",
        workspace,
        ["cargo", "mock"],
    )

    assert fake_local.cwd_calls == [crate_dir]
    assert fake_local.env_calls == [
        {"CARGO_HOME": str(workspace / ".cargo-home")}
    ]
    assert fake_local.invocations == [(["cargo", "mock"], 11)]


def test_run_cargo_command_logs_failures(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    caplog: pytest.LogCaptureFixture,
    run_publish_check_module,
) -> None:
    workspace = tmp_path / "workspace"
    workspace.mkdir()
    crate_dir = workspace / "crates" / "demo"
    crate_dir.mkdir(parents=True)

    fake_local = FakeLocal(lambda _args, _timeout: (3, "bad stdout", "bad stderr"))
    monkeypatch.setattr(run_publish_check_module, "local", fake_local)

    with caplog.at_level("ERROR"):
        with pytest.raises(SystemExit) as excinfo:
            run_publish_check_module.run_cargo_command(
                "demo",
                workspace,
                ["cargo", "failing"],
                timeout_secs=5,
            )
    assert "exit code 3" in str(excinfo.value)
    assert "bad stdout" in caplog.text
    assert "bad stderr" in caplog.text


def test_run_cargo_command_passes_command_result(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    run_publish_check_module,
) -> None:
    workspace = tmp_path / "workspace"
    workspace.mkdir()
    crate_dir = workspace / "crates" / "demo"
    crate_dir.mkdir(parents=True)

    fake_local = FakeLocal(lambda _args, _timeout: (5, "out", "err"))
    monkeypatch.setattr(run_publish_check_module, "local", fake_local)

    observed: dict[str, object] = {}

    def record_failure(crate: str, result: object) -> None:
        observed["crate"] = crate
        observed["result"] = result
        raise SystemExit("handler invoked")

    monkeypatch.setattr(run_publish_check_module, "_handle_command_failure", record_failure)

    with pytest.raises(SystemExit, match="handler invoked"):
        run_publish_check_module.run_cargo_command(
            "demo",
            workspace,
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
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    run_publish_check_module,
) -> None:
    workspace = tmp_path / "workspace"
    workspace.mkdir()
    crate_dir = workspace / "crates" / "demo"
    crate_dir.mkdir(parents=True)

    def raise_timeout(_args: list[str], _timeout: int | None) -> tuple[int, str, str]:
        raise run_publish_check_module.ProcessTimedOut("timeout", _args)

    fake_local = FakeLocal(raise_timeout)
    monkeypatch.setattr(run_publish_check_module, "local", fake_local)

    with pytest.raises(SystemExit) as excinfo:
        run_publish_check_module.run_cargo_command(
            "demo",
            workspace,
            ["cargo", "wait"],
            timeout_secs=1,
        )
    assert "timed out" in str(excinfo.value)


def test_package_crate_invokes_cargo_with_timeout(
    monkeypatch: pytest.MonkeyPatch,
    run_publish_check_module,
) -> None:
    calls: list[tuple[str, Path, list[str], int]] = []

    def fake_run_cargo(
        crate: str,
        workspace_root: Path,
        command: list[str],
        *,
        timeout_secs: int,
    ) -> None:
        calls.append((crate, workspace_root, command, timeout_secs))

    monkeypatch.setattr(run_publish_check_module, "run_cargo_command", fake_run_cargo)

    workspace = Path("/tmp/workspace")
    run_publish_check_module.package_crate("demo", workspace, timeout_secs=42)

    assert calls == [
        (
            "demo",
            workspace,
            ["cargo", "package", "--allow-dirty", "--no-verify"],
            42,
        )
    ]


def test_check_crate_invokes_cargo_with_timeout(
    monkeypatch: pytest.MonkeyPatch,
    run_publish_check_module,
) -> None:
    calls: list[tuple[str, Path, list[str], int]] = []

    def fake_run_cargo(
        crate: str,
        workspace_root: Path,
        command: list[str],
        *,
        timeout_secs: int,
    ) -> None:
        calls.append((crate, workspace_root, command, timeout_secs))

    monkeypatch.setattr(run_publish_check_module, "run_cargo_command", fake_run_cargo)

    workspace = Path("/tmp/workspace")
    run_publish_check_module.check_crate("demo", workspace, timeout_secs=17)

    assert calls == [
        (
            "demo",
            workspace,
            ["cargo", "check", "--all-features"],
            17,
        )
    ]


def test_run_publish_check_orchestrates_workflow(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    run_publish_check_module,
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
    monkeypatch.setattr(
        run_publish_check_module,
        "apply_workspace_replacements",
        lambda root, version: steps.append(("apply", (root, version))),
    )
    monkeypatch.setattr(run_publish_check_module, "package_crate", fake_package)
    monkeypatch.setattr(run_publish_check_module, "check_crate", fake_check)
    monkeypatch.setattr(
        run_publish_check_module,
        "PUBLISH_CRATES",
        ["rstest-bdd-patterns", "demo-crate"],
    )

    run_publish_check_module.run_publish_check(keep_tmp=False, timeout_secs=15)

    manifest_path = workspace_dir / "Cargo.toml"
    assert steps[:3] == [
        ("export", workspace_dir),
        ("prune", manifest_path),
        ("strip", manifest_path),
    ]
    assert ("version", manifest_path) in steps
    assert ("apply", (workspace_dir, "9.9.9")) in steps
    assert package_calls == [("rstest-bdd-patterns", workspace_dir, 15)]
    assert check_calls == [("demo-crate", workspace_dir, 15)]
    assert not workspace_dir.exists()


def test_run_publish_check_preserves_workspace(monkeypatch, tmp_path: Path, capsys, run_publish_check_module) -> None:
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
    monkeypatch.setattr(run_publish_check_module, "PUBLISH_CRATES", [])

    run_publish_check_module.run_publish_check(keep_tmp=True, timeout_secs=5)

    captured = capsys.readouterr()
    assert "preserving workspace" in captured.out
    assert workspace_dir.exists()


def test_run_publish_check_rejects_non_positive_timeout(
    run_publish_check_module,
) -> None:
    with pytest.raises(SystemExit, match="positive integer"):
        run_publish_check_module.run_publish_check(keep_tmp=False, timeout_secs=0)


def test_main_uses_defaults(
    monkeypatch: pytest.MonkeyPatch,
    run_publish_check_module,
) -> None:
    captured: dict[str, object] = {}

    def fake_run_publish_check(*, keep_tmp: bool, timeout_secs: int) -> None:
        captured["keep_tmp"] = keep_tmp
        captured["timeout_secs"] = timeout_secs

    monkeypatch.setattr(run_publish_check_module, "run_publish_check", fake_run_publish_check)

    run_publish_check_module.app([])

    assert captured == {
        "keep_tmp": False,
        "timeout_secs": run_publish_check_module.DEFAULT_PUBLISH_TIMEOUT_SECS,
    }


def test_main_honours_environment(monkeypatch, run_publish_check_module) -> None:
    observed: list[tuple[bool, int]] = []

    def fake_run_publish_check(*, keep_tmp: bool, timeout_secs: int) -> None:
        observed.append((keep_tmp, timeout_secs))

    monkeypatch.setattr(run_publish_check_module, "run_publish_check", fake_run_publish_check)
    monkeypatch.setenv("PUBLISH_CHECK_KEEP_TMP", "true")
    monkeypatch.setenv("PUBLISH_CHECK_TIMEOUT_SECS", "60")

    run_publish_check_module.app([])

    assert observed == [(True, 60)]


def test_main_cli_overrides_env(monkeypatch, run_publish_check_module) -> None:
    observed: list[tuple[bool, int]] = []

    def fake_run_publish_check(*, keep_tmp: bool, timeout_secs: int) -> None:
        observed.append((keep_tmp, timeout_secs))

    monkeypatch.setattr(run_publish_check_module, "run_publish_check", fake_run_publish_check)
    monkeypatch.setenv("PUBLISH_CHECK_KEEP_TMP", "false")
    monkeypatch.setenv("PUBLISH_CHECK_TIMEOUT_SECS", "900")

    run_publish_check_module.app(["--keep-tmp", "--timeout-secs", "5"])

    assert observed == [(True, 5)]
