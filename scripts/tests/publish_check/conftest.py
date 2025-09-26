"""Shared fixtures and helper fakes for publish check workflows."""

from __future__ import annotations

import contextlib
import importlib.util
import sys
from dataclasses import dataclass
from pathlib import Path
from types import ModuleType
from typing import Callable, Sequence

import pytest

SCRIPTS_DIR = Path(__file__).resolve().parents[2]
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

RunCallable = Callable[[Sequence[str], int | None], tuple[int, str, str]]


@dataclass(frozen=True)
class CommandFailureTestCase:
    crate: str
    result_kwargs: dict[str, object]
    expected_exit_fragment: str | None
    expected_logs: tuple[str, ...]
    unexpected_logs: tuple[str, ...]


@dataclass(frozen=True)
class CargoTestContext:
    """Test context container for cargo command scenarios."""

    patch_local_runner: Callable[[RunCallable], "FakeLocal"]
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


@pytest.fixture(scope="module")
def publish_workspace_module(run_publish_check_module: ModuleType) -> ModuleType:
    module = sys.modules.get("publish_workspace")
    assert module is not None
    return module


@pytest.fixture
def fake_workspace(tmp_path: Path) -> Path:
    """Provision a fake workspace tree used by cargo command tests."""

    workspace = tmp_path / "workspace"
    (workspace / "crates" / "demo").mkdir(parents=True)
    return workspace


@pytest.fixture
def mock_cargo_runner(
    monkeypatch: pytest.MonkeyPatch, run_publish_check_module: ModuleType
) -> list[tuple[str, Path, tuple[str, ...], int]]:
    """Capture invocations made to ``run_cargo_command``."""

    calls: list[tuple[str, Path, tuple[str, ...], int]] = []

    def fake_run_cargo(
        crate: str,
        workspace_root: Path,
        command: Sequence[str],
        *,
        timeout_secs: int,
    ) -> None:
        calls.append((crate, workspace_root, tuple(command), timeout_secs))

    monkeypatch.setattr(run_publish_check_module, "run_cargo_command", fake_run_cargo)
    return calls


@pytest.fixture
def patch_local_runner(
    monkeypatch: pytest.MonkeyPatch, run_publish_check_module: ModuleType
) -> Callable[[RunCallable], "FakeLocal"]:
    """Install a ``FakeLocal`` around the provided callable."""

    def _install(run_callable: RunCallable) -> "FakeLocal":
        fake_local = FakeLocal(run_callable)
        monkeypatch.setattr(run_publish_check_module, "local", fake_local)
        return fake_local

    return _install


@pytest.fixture
def cargo_test_context(
    patch_local_runner: Callable[[RunCallable], "FakeLocal"],
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
    def __init__(self, local: "FakeLocal", args: Sequence[str]):
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
