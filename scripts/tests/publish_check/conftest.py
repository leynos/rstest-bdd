"""Shared fixtures and helper fakes for publish check workflows."""

from __future__ import annotations

import contextlib
import dataclasses as dc
import importlib.util
import sys
import typing as typ
from pathlib import Path

import pytest

if typ.TYPE_CHECKING:
    from types import ModuleType

SCRIPTS_DIR = Path(__file__).resolve().parents[2]
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

RunCallable = typ.Callable[[list[str], int | None], tuple[int, str, str]]


@dc.dataclass(frozen=True)
class CommandFailureTestCase:
    """Describe an expected crate failure and associated log fragments."""

    crate: str
    result_kwargs: dict[str, object]
    expected_exit_fragment: str | None
    expected_logs: tuple[str, ...]
    unexpected_logs: tuple[str, ...]


@dc.dataclass(frozen=True)
class CargoTestContext:
    """Test context container for cargo command scenarios."""

    patch_local_runner: typ.Callable[[RunCallable], FakeLocal]
    fake_workspace: Path
    caplog: pytest.LogCaptureFixture
    run_publish_check_module: ModuleType


@pytest.fixture(scope="module")
def run_publish_check_module() -> ModuleType:
    """Load ``run_publish_check`` as a real module for integration tests."""
    spec = importlib.util.spec_from_file_location(
        "run_publish_check", SCRIPTS_DIR / "run_publish_check.py"
    )
    if spec is None or spec.loader is None:
        msg = "Failed to locate run_publish_check module spec"
        raise RuntimeError(msg)

    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


@pytest.fixture(scope="module")
def publish_workspace_module() -> ModuleType:
    """Load ``publish_workspace`` as a module for integration tests."""
    spec = importlib.util.spec_from_file_location(
        "publish_workspace", SCRIPTS_DIR / "publish_workspace.py"
    )
    if spec is None or spec.loader is None:
        msg = "Failed to locate publish_workspace module spec"
        raise RuntimeError(msg)

    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
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
) -> list[tuple[str, Path, list[str], int]]:
    """Capture invocations made to ``run_cargo_command``."""
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
) -> typ.Callable[[RunCallable], FakeLocal]:
    """Install a ``FakeLocal`` around the provided callable."""

    def _install(run_callable: RunCallable) -> FakeLocal:
        fake_local = FakeLocal(run_callable)
        monkeypatch.setattr(run_publish_check_module, "local", fake_local)
        return fake_local

    return _install


@pytest.fixture
def cargo_test_context(
    patch_local_runner: typ.Callable[[RunCallable], FakeLocal],
    fake_workspace: Path,
    caplog: pytest.LogCaptureFixture,
    run_publish_check_module: ModuleType,
) -> CargoTestContext:
    """Bundle fixtures required for cargo command assertions."""
    return CargoTestContext(
        patch_local_runner=patch_local_runner,
        fake_workspace=fake_workspace,
        caplog=caplog,
        run_publish_check_module=run_publish_check_module,
    )


class FakeCargoInvocation:
    """Record a cargo invocation and proxy execution to the fake runner."""

    def __init__(self, local: FakeLocal, args: list[str]) -> None:
        """Store the invocation context for later assertions."""
        self._local = local
        self._args = ["cargo", *args]

    def run(
        self, *, retcode: object | None, timeout: int | None
    ) -> tuple[int, str, str]:
        """Record an invocation and delegate to the configured callable."""
        self._local.invocations.append((self._args, timeout))
        return self._local.run_callable(self._args, timeout)


class FakeCargo:
    """Proxy indexing calls into ``FakeCargoInvocation`` instances."""

    def __init__(self, local: FakeLocal) -> None:
        """Initialise the cargo proxy for a fake local environment."""
        self._local = local

    def __getitem__(self, args: object) -> FakeCargoInvocation:
        """Return an invocation wrapper for the provided command arguments."""
        extras = list(args) if isinstance(args, (list, tuple)) else [str(args)]
        return FakeCargoInvocation(self._local, extras)


class FakeLocal:
    """Mimic a fabric ``local`` helper for cargo orchestration tests."""

    def __init__(self, run_callable: RunCallable) -> None:
        """Store the callable that will service fake local invocations."""
        self.run_callable = run_callable
        self.cwd_calls: list[Path] = []
        self.env_calls: list[dict[str, str]] = []
        self.invocations: list[tuple[list[str], int | None]] = []

    def __getitem__(self, command: str) -> FakeCargo:
        """Return a ``FakeCargo`` proxy for the ``cargo`` command."""
        if command != "cargo":
            msg = (
                f"FakeLocal only understands the 'cargo' command, received {command!r}"
            )
            raise RuntimeError(msg)
        return FakeCargo(self)

    def cwd(self, path: Path) -> contextlib.AbstractContextManager[None]:
        """Record the working directory change for later assertions."""
        self.cwd_calls.append(path)
        return contextlib.nullcontext()

    def env(self, **kwargs: str) -> contextlib.AbstractContextManager[None]:
        """Record environment mutations for later assertions."""
        self.env_calls.append(kwargs)
        return contextlib.nullcontext()
