"""Validate scripts.run_publish_check end-to-end.

The suite covers cargo invocation handling, timeout propagation, error
reporting, and the temporary workspace export and pruning steps performed
before packaging so the publish check remains safe. Tests are expected to run
under pytest with local fakes, ensuring release automation can be exercised
without invoking real tooling.
"""

from __future__ import annotations

import contextlib
import importlib.util
import sys
from dataclasses import dataclass
from pathlib import Path
from types import ModuleType
from typing import Callable

import pytest

SCRIPTS_DIR = Path(__file__).resolve().parents[1]
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))


RunCallable = Callable[[list[str], int | None], tuple[int, str, str]]


@dataclass(frozen=True)
class CommandFailureTestCase:
    crate: str
    result_kwargs: dict[str, object]
    expected_exit_fragment: str | None
    expected_logs: tuple[str, ...]
    unexpected_logs: tuple[str, ...]


@dataclass(frozen=True)
class CargoTestContext:
    """Test context for cargo command testing scenarios."""

    patch_local_runner: Callable[[RunCallable], FakeLocal]
    fake_workspace: Path
    caplog: pytest.LogCaptureFixture
    run_publish_check_module: ModuleType


@pytest.fixture(scope="module")
def run_publish_check_module() -> ModuleType:
    spec = importlib.util.spec_from_file_location(
        "run_publish_check", SCRIPTS_DIR / "run_publish_check.py"
    )
    module = importlib.util.module_from_spec(spec)
    assert spec and spec.loader
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


@pytest.fixture
def fake_workspace(tmp_path: Path) -> Path:
    """Provision a fake workspace tree used by cargo command tests.

    Parameters
    ----------
    tmp_path : Path
        Pytest-provided temporary directory for the current test invocation.

    Returns
    -------
    Path
        Root path of the workspace with a ``demo`` crate directory in place.
    """

    workspace = tmp_path / "workspace"
    (workspace / "crates" / "demo").mkdir(parents=True)
    return workspace


@pytest.fixture
def mock_cargo_runner(
    monkeypatch: pytest.MonkeyPatch, run_publish_check_module: ModuleType
) -> list[tuple[str, Path, list[str], int]]:
    """Capture invocations made to ``run_cargo_command``.

    Parameters
    ----------
    monkeypatch : pytest.MonkeyPatch
        Fixture used to patch the ``run_cargo_command`` helper for inspection.
    run_publish_check_module : ModuleType
        Loaded ``run_publish_check`` module that exposes the helper.

    Returns
    -------
    list[tuple[str, Path, list[str], int]]
        Recorded invocations with the crate name, workspace, command, and
        timeout seconds.
    """

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
    return calls


@pytest.fixture
def patch_local_runner(
    monkeypatch: pytest.MonkeyPatch, run_publish_check_module: ModuleType
) -> Callable[[RunCallable], "FakeLocal"]:
    """Install a ``FakeLocal`` around the provided callable.

    Parameters
    ----------
    monkeypatch : pytest.MonkeyPatch
        Fixture used to patch the module under test.
    run_publish_check_module : ModuleType
        Loaded ``run_publish_check`` module that exposes the Fabric ``local``.

    Returns
    -------
    Callable[[RunCallable], FakeLocal]
        Factory that applies the patch and yields the configured ``FakeLocal``.
    """

    def _install(run_callable: RunCallable) -> "FakeLocal":
        fake_local = FakeLocal(run_callable)
        monkeypatch.setattr(run_publish_check_module, "local", fake_local)
        return fake_local

    return _install


@pytest.fixture
def cargo_test_context(
    patch_local_runner: Callable[[RunCallable], FakeLocal],
    fake_workspace: Path,
    caplog: pytest.LogCaptureFixture,
    run_publish_check_module: ModuleType,
) -> CargoTestContext:
    return CargoTestContext(
        patch_local_runner=patch_local_runner,
        fake_workspace=fake_workspace,
        caplog=caplog,
        run_publish_check_module=run_publish_check_module,
    )


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
    def __init__(self, run_callable: RunCallable):
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
    result = run_publish_check_module.CommandResult(**test_case.result_kwargs)

    with caplog.at_level("ERROR"):
        with pytest.raises(SystemExit) as excinfo:
            run_publish_check_module._handle_command_failure(test_case.crate, result)

    if test_case.expected_exit_fragment is not None:
        assert test_case.expected_exit_fragment in str(excinfo.value)

    for text in test_case.expected_logs:
        assert text in caplog.text

    for text in test_case.unexpected_logs:
        assert text not in caplog.text


def test_run_cargo_command_streams_output(
    patch_local_runner: Callable[[RunCallable], FakeLocal],
    fake_workspace: Path,
    capsys: pytest.CaptureFixture[str],
    run_publish_check_module: ModuleType,
) -> None:
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
    assert fake_local.env_calls == [
        {"CARGO_HOME": str(fake_workspace / ".cargo-home")}
    ]
    assert fake_local.invocations == [(["cargo", "mock"], 5)]


def test_run_cargo_command_uses_env_timeout(
    monkeypatch: pytest.MonkeyPatch,
    patch_local_runner: Callable[[RunCallable], FakeLocal],
    fake_workspace: Path,
    run_publish_check_module: ModuleType,
) -> None:
    crate_dir = fake_workspace / "crates" / "demo"

    fake_local = patch_local_runner(lambda _args, timeout: (0, "", ""))
    monkeypatch.setenv("PUBLISH_CHECK_TIMEOUT_SECS", "11")

    run_publish_check_module.run_cargo_command(
        "demo",
        fake_workspace,
        ["cargo", "mock"],
    )

    assert fake_local.cwd_calls == [crate_dir]
    assert fake_local.env_calls == [
        {"CARGO_HOME": str(fake_workspace / ".cargo-home")}
    ]
    assert fake_local.invocations == [(["cargo", "mock"], 11)]


def test_run_cargo_command_logs_failures(
    monkeypatch: pytest.MonkeyPatch,
    context: CargoTestContext,
) -> None:
    fake_local = context.patch_local_runner(
        lambda _args, _timeout: (3, "bad stdout", "bad stderr")
    )

    with context.caplog.at_level("ERROR"):
        with pytest.raises(SystemExit) as excinfo:
            context.run_publish_check_module.run_cargo_command(
                "demo",
                context.fake_workspace,
                ["cargo", "failing"],
                timeout_secs=5,
            )
    assert "exit code 3" in str(excinfo.value)
    assert "bad stdout" in context.caplog.text
    assert "bad stderr" in context.caplog.text
    assert fake_local.cwd_calls == [context.fake_workspace / "crates" / "demo"]


def test_run_cargo_command_passes_command_result(
    monkeypatch: pytest.MonkeyPatch,
    patch_local_runner: Callable[[RunCallable], FakeLocal],
    fake_workspace: Path,
    run_publish_check_module: ModuleType,
) -> None:
    fake_local = patch_local_runner(lambda _args, _timeout: (5, "out", "err"))

    observed: dict[str, object] = {}

    def record_failure(crate: str, result: object) -> None:
        observed["crate"] = crate
        observed["result"] = result
        raise SystemExit("handler invoked")

    monkeypatch.setattr(run_publish_check_module, "_handle_command_failure", record_failure)

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
    patch_local_runner: Callable[[RunCallable], FakeLocal],
    fake_workspace: Path,
    run_publish_check_module: ModuleType,
) -> None:
    def raise_timeout(_args: list[str], _timeout: int | None) -> tuple[int, str, str]:
        raise run_publish_check_module.ProcessTimedOut("timeout", _args)

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
    function_name, expected_command = function_and_command
    crate, timeout = test_scenario
    workspace = Path("/tmp/workspace")

    getattr(run_publish_check_module, function_name)(
        crate, workspace, timeout_secs=timeout
    )

    assert mock_cargo_runner == [(crate, workspace, expected_command, timeout)]


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
