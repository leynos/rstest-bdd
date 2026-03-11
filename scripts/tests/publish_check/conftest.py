"""Shared fixtures and helper fakes for publish check workflows."""

from __future__ import annotations

import dataclasses as dc
import importlib.util
import sys
import typing as typ
from pathlib import Path

import pytest

from .fakes import FakeLocal, RunCallable

if typ.TYPE_CHECKING:
    from types import ModuleType

SCRIPTS_DIR = Path(__file__).resolve().parents[2]
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))


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


@dc.dataclass
class WorkspaceMocks:
    """Bundle of mock functions for workspace operations."""

    record: typ.Callable[[str], typ.Callable[[Path], None]]
    fake_apply: typ.Callable[..., None]
    fake_remove: typ.Callable[[Path, str], None]


@dc.dataclass
class WorkflowTestConfig:
    """Configuration bundle for workflow integration scaffolding."""

    workspace_name: str
    crate_order: tuple[str, ...] = ("demo-crate",)


@dc.dataclass(frozen=True)
class CrateActionCalls:
    """Record crate-action invocations.

    Attributes
    ----------
    package, gpui, check : list[tuple[str, Path, int]]
    """

    package: list[tuple[str, Path, int]]
    gpui: list[tuple[str, Path, int]]
    check: list[tuple[str, Path, int]]


@dc.dataclass(frozen=True)
class GpuiPackagePaths:
    """Describe GPUI package-check paths.

    Attributes
    ----------
    archive, package_dir, validator_dir : Path
    """

    archive: Path
    package_dir: Path
    validator_dir: Path


@dc.dataclass(frozen=True)
class GpuiHarnessPatchState:
    """Capture GPUI harness patch side effects.

    Attributes
    ----------
    steps : list[tuple[str, object]]
    workspace_version_args : list[Path]
    packaged_archive_path_args : list[tuple[Path, str, str]]
    """

    steps: list[tuple[str, object]]
    workspace_version_args: list[Path]
    packaged_archive_path_args: list[tuple[Path, str, str]]


def _load_module_from_scripts(module_name: str, script_filename: str) -> ModuleType:
    """Load ``module_name`` from ``scripts`` while guarding against import issues."""
    script_path = SCRIPTS_DIR / script_filename
    spec = importlib.util.spec_from_file_location(module_name, script_path)
    if spec is None or spec.loader is None:
        msg = f"Failed to load module spec for {module_name!r} from {script_path}"
        raise RuntimeError(msg)

    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


@pytest.fixture(scope="module")
def run_publish_check_module() -> ModuleType:
    """Load ``run_publish_check`` as a real module for integration tests."""
    return _load_module_from_scripts("run_publish_check", "run_publish_check.py")


@pytest.fixture(scope="module")
def publish_workspace_module() -> ModuleType:
    """Load ``publish_workspace`` as a module for integration tests."""
    return _load_module_from_scripts("publish_workspace", "publish_workspace.py")


@pytest.fixture
def crate_action_calls(
    monkeypatch: pytest.MonkeyPatch,
    run_publish_check_module: ModuleType,
) -> CrateActionCalls:
    """Install crate-action recorders.

    Parameters
    ----------
    monkeypatch : pytest.MonkeyPatch
    run_publish_check_module : ModuleType

    Returns
    -------
    CrateActionCalls
    """
    calls = CrateActionCalls(package=[], gpui=[], check=[])
    monkeypatch.setattr(
        run_publish_check_module,
        "package_crate",
        lambda crate, root, *, timeout_secs: calls.package.append(
            (crate, root, timeout_secs)
        ),
    )
    monkeypatch.setattr(
        run_publish_check_module,
        "check_crate",
        lambda crate, root, *, timeout_secs: calls.check.append(
            (crate, root, timeout_secs)
        ),
    )
    monkeypatch.setattr(
        run_publish_check_module,
        "validate_packaged_gpui_harness",
        lambda crate, root, *, timeout_secs: calls.gpui.append(
            (crate, root, timeout_secs)
        ),
    )
    return calls


def _build_gpui_patch_state(
    paths: GpuiPackagePaths,
    monkeypatch: pytest.MonkeyPatch,
    mod: ModuleType,
) -> GpuiHarnessPatchState:
    """Initialise recorder state, register patches on ``mod``, and return the state."""
    steps: list[tuple[str, object]] = []
    workspace_version_args: list[Path] = []
    packaged_archive_path_args: list[tuple[Path, str, str]] = []

    def record_workspace_version(manifest: Path) -> str:
        workspace_version_args.append(manifest)
        return "1.2.3"

    def record_build_packaged_archive(
        root: Path,
        archive_path: Path,
        version: str,
        *,
        timeout_secs: int | None = None,
    ) -> None:
        steps.append(("archive", (root, archive_path, version, timeout_secs)))

    def record_packaged_archive_path(root: Path, crate: str, version: str) -> Path:
        packaged_archive_path_args.append((root, crate, version))
        return paths.archive

    def fake_extract_packaged_archive(archive_path: Path, destination: Path) -> Path:
        steps.append(("extract", (archive_path, destination)))
        return paths.package_dir

    def fake_write_validator_workspace(
        destination: Path,
        *,
        package_dir: Path,
        harness_dir: Path,
        version: str,
    ) -> Path:
        steps.append(("validator", (destination, package_dir, harness_dir, version)))
        return paths.validator_dir

    monkeypatch.setattr(mod, "workspace_version", record_workspace_version)
    monkeypatch.setattr(mod, "build_packaged_archive", record_build_packaged_archive)
    monkeypatch.setattr(mod, "packaged_archive_path", record_packaged_archive_path)
    monkeypatch.setattr(mod, "extract_packaged_archive", fake_extract_packaged_archive)
    monkeypatch.setattr(
        mod, "write_validator_workspace", fake_write_validator_workspace
    )
    monkeypatch.setattr(
        mod,
        "run_cargo_command",
        lambda context, command: steps.append(("cargo", (context, list(command)))),
    )

    return GpuiHarnessPatchState(
        steps=steps,
        workspace_version_args=workspace_version_args,
        packaged_archive_path_args=packaged_archive_path_args,
    )


@pytest.fixture
def gpui_harness_calls(
    monkeypatch: pytest.MonkeyPatch,
    run_publish_check_module: ModuleType,
) -> typ.Callable[[GpuiPackagePaths], GpuiHarnessPatchState]:
    """Install GPUI harness recorders.

    Parameters
    ----------
    monkeypatch : pytest.MonkeyPatch
    run_publish_check_module : ModuleType

    Returns
    -------
    Callable[[GpuiPackagePaths], GpuiHarnessPatchState]
    """
    return lambda paths: _build_gpui_patch_state(
        paths, monkeypatch, run_publish_check_module
    )


@pytest.fixture
def fake_workspace(tmp_path: Path) -> Path:
    """Provision a fake workspace tree.

    Parameters
    ----------
    tmp_path : Path

    Returns
    -------
    Path
    """
    workspace = tmp_path / "workspace"
    (workspace / "crates" / "demo").mkdir(parents=True)
    return workspace


@pytest.fixture
def mock_cargo_runner(
    monkeypatch: pytest.MonkeyPatch, run_publish_check_module: ModuleType
) -> list[tuple[object, list[str], typ.Callable[[str, object], bool] | None]]:
    """Capture ``run_cargo_command`` invocations.

    Parameters
    ----------
    monkeypatch : pytest.MonkeyPatch
    run_publish_check_module : ModuleType

    Returns
    -------
    list[tuple[object, list[str], Callable[[str, object], bool] | None]]
    """
    calls: list[tuple[object, list[str], typ.Callable[[str, object], bool] | None]] = []

    def fake_run_cargo(
        context: run_publish_check_module.CargoCommandContext,
        command: list[str],
        *,
        on_failure: typ.Callable[[str, run_publish_check_module.CommandResult], bool]
        | None = None,
    ) -> None:
        calls.append((context, command, on_failure))

    monkeypatch.setattr(run_publish_check_module, "run_cargo_command", fake_run_cargo)
    return calls


@pytest.fixture
def patch_local_runner(
    monkeypatch: pytest.MonkeyPatch, run_publish_check_module: ModuleType
) -> typ.Callable[[RunCallable], FakeLocal]:
    """Install ``FakeLocal`` wrappers.

    Parameters
    ----------
    monkeypatch : pytest.MonkeyPatch
    run_publish_check_module : ModuleType

    Returns
    -------
    Callable[[RunCallable], FakeLocal]
    """

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
    """Bundle fixtures for cargo command assertions.

    Parameters
    ----------
    patch_local_runner : Callable[[RunCallable], FakeLocal]
    fake_workspace : Path
    caplog : pytest.LogCaptureFixture
    run_publish_check_module : ModuleType

    Returns
    -------
    CargoTestContext
    """
    return CargoTestContext(
        patch_local_runner=patch_local_runner,
        fake_workspace=fake_workspace,
        caplog=caplog,
        run_publish_check_module=run_publish_check_module,
    )


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
