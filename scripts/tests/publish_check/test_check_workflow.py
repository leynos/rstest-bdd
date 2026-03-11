"""Unit tests covering the publish-check validation workflow."""

from __future__ import annotations

import dataclasses
import typing as typ

import pytest

if typ.TYPE_CHECKING:
    from pathlib import Path
    from types import ModuleType


@dataclasses.dataclass(frozen=True)
class _CrateActionCalls:
    package: list[tuple[str, Path, int]]
    gpui: list[tuple[str, Path, int]]
    check: list[tuple[str, Path, int]]


def _patch_crate_action_functions(
    monkeypatch: pytest.MonkeyPatch,
    mod: ModuleType,
) -> _CrateActionCalls:
    """Register crate-action monkeypatches and return the recorded call lists."""
    calls = _CrateActionCalls(package=[], gpui=[], check=[])
    monkeypatch.setattr(
        mod,
        "package_crate",
        lambda crate, root, *, timeout_secs: calls.package.append(
            (crate, root, timeout_secs)
        ),
    )
    monkeypatch.setattr(
        mod,
        "check_crate",
        lambda crate, root, *, timeout_secs: calls.check.append(
            (crate, root, timeout_secs)
        ),
    )
    monkeypatch.setattr(
        mod,
        "validate_packaged_gpui_harness",
        lambda crate, root, *, timeout_secs: calls.gpui.append(
            (crate, root, timeout_secs)
        ),
    )
    return calls


def test_process_crates_for_check_delegates_configuration(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    run_publish_check_module: ModuleType,
) -> None:
    """Ensure the check flow supplies the expected processing configuration."""
    observed: dict[str, object] = {}
    calls = _patch_crate_action_functions(monkeypatch, run_publish_check_module)

    def fake_process_crates(
        workspace: Path,
        timeout_secs: int,
        config: object,
        crate_action: object,
    ) -> None:
        observed["workspace"] = workspace
        observed["timeout"] = timeout_secs
        observed["config"] = config
        observed["crate_action"] = crate_action
        action = crate_action
        action("rstest-bdd-patterns", workspace, timeout_secs=11)
        action("rstest-bdd-harness-gpui", workspace, timeout_secs=11)
        action("demo", workspace, timeout_secs=11)

    monkeypatch.setattr(
        run_publish_check_module, "_process_crates", fake_process_crates
    )

    workspace = tmp_path / "check"
    run_publish_check_module._process_crates_for_check(workspace, 17)

    assert observed["workspace"] == workspace, (
        f"workspace mismatch: expected {workspace=}, got {observed['workspace']=}"
    )
    assert observed["timeout"] == 17, (
        f"timeout mismatch: expected 17, got {observed['timeout']=}"
    )
    config = observed["config"]
    assert isinstance(config, run_publish_check_module.CrateProcessingConfig), (
        f"expected crate processing config type, got {type(config)=}"
    )
    assert config.strip_patch is True, (
        f"expected {config.strip_patch=} for stripped patch configuration"
    )
    assert config.include_local_path is True, (
        f"expected {config.include_local_path=} for local path inclusion"
    )
    assert config.apply_per_crate is False, (
        f"expected {config.apply_per_crate=} for shared workspace processing"
    )
    assert config.per_crate_cleanup is None, (
        f"expected {config.per_crate_cleanup=} cleanup hook"
    )
    assert callable(observed["crate_action"]), (
        f"expected callable crate_action, got {observed['crate_action']=}"
    )
    assert calls.package == [("rstest-bdd-patterns", workspace, 11)], (
        f"expected calls.package for patterns crate, got {calls.package=}"
    )
    assert calls.gpui == [("rstest-bdd-harness-gpui", workspace, 11)], (
        f"expected calls.gpui for GPUI harness, got {calls.gpui=}"
    )
    assert calls.check == [("demo", workspace, 11)], (
        f"expected calls.check for demo crate, got {calls.check=}"
    )


def test_process_crates_for_check_runs_local_validation(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    run_publish_check_module: ModuleType,
) -> None:
    """Validate that the check flow packages and checks the configured crates."""
    workspace = tmp_path / "workspace"
    workspace.mkdir()
    (workspace / "Cargo.toml").write_text("", encoding="utf-8")

    steps: list[tuple[str, object]] = []

    def fake_strip(manifest: Path) -> None:
        steps.append(("strip", manifest))

    monkeypatch.setattr(run_publish_check_module, "strip_patch_section", fake_strip)
    monkeypatch.setattr(
        run_publish_check_module, "workspace_version", lambda _m: "9.9.9"
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
        run_publish_check_module, "apply_workspace_replacements", fake_apply
    )

    def fake_package(crate: str, root: Path, *, timeout_secs: int) -> None:
        steps.append(("package", (crate, root, timeout_secs)))

    def fake_check(crate: str, root: Path, *, timeout_secs: int) -> None:
        steps.append(("check", (crate, root, timeout_secs)))

    def fake_gpui(crate: str, root: Path, *, timeout_secs: int) -> None:
        steps.append(("gpui", (crate, root, timeout_secs)))

    monkeypatch.setattr(run_publish_check_module, "package_crate", fake_package)
    monkeypatch.setattr(run_publish_check_module, "check_crate", fake_check)
    monkeypatch.setattr(
        run_publish_check_module, "validate_packaged_gpui_harness", fake_gpui
    )
    monkeypatch.setattr(
        run_publish_check_module,
        "CRATE_ORDER",
        ("rstest-bdd-patterns", "rstest-bdd-harness-gpui", "crate-b"),
    )

    run_publish_check_module._process_crates_for_check(workspace, 55)

    manifest = workspace / "Cargo.toml"
    assert steps == [
        ("strip", manifest),
        ("apply", (workspace, "9.9.9", True, None)),
        ("package", ("rstest-bdd-patterns", workspace, 55)),
        ("gpui", ("rstest-bdd-harness-gpui", workspace, 55)),
        ("check", ("crate-b", workspace, 55)),
    ], f"expected strip/apply/package/gpui/check workflow steps, got {steps=}"


@dataclasses.dataclass(frozen=True)
class _GpuiPackagePaths:
    archive: Path
    package_dir: Path
    validator_dir: Path


@dataclasses.dataclass(frozen=True)
class _GpuiHarnessPatchState:
    steps: list[tuple[str, object]]
    workspace_version_args: list[Path]
    packaged_archive_path_args: list[tuple[Path, str, str]]


def _patch_gpui_harness_functions(
    monkeypatch: pytest.MonkeyPatch,
    mod: ModuleType,
    paths: _GpuiPackagePaths,
) -> _GpuiHarnessPatchState:
    """Register GPUI harness monkeypatches and return the recorded steps."""
    steps: list[tuple[str, object]] = []
    workspace_version_args: list[Path] = []
    packaged_archive_path_args: list[tuple[Path, str, str]] = []

    def record_build_packaged_archive(
        root: Path,
        archive_path: Path,
        version: str,
        *,
        timeout_secs: int | None = None,
    ) -> None:
        steps.append(("archive", (root, archive_path, version, timeout_secs)))

    def record_workspace_version(manifest: Path) -> str:
        workspace_version_args.append(manifest)
        return "1.2.3"

    def record_packaged_archive_path(root: Path, crate: str, version: str) -> Path:
        packaged_archive_path_args.append((root, crate, version))
        return paths.archive

    monkeypatch.setattr(mod, "workspace_version", record_workspace_version)
    monkeypatch.setattr(mod, "build_packaged_archive", record_build_packaged_archive)
    monkeypatch.setattr(
        mod,
        "packaged_archive_path",
        record_packaged_archive_path,
    )
    monkeypatch.setattr(
        mod,
        "extract_packaged_archive",
        lambda archive_path, destination: (
            steps.append(("extract", (archive_path, destination))) or paths.package_dir
        ),
    )
    monkeypatch.setattr(
        mod,
        "write_validator_workspace",
        lambda destination, *, package_dir, harness_dir, version: (
            steps.append(
                ("validator", (destination, package_dir, harness_dir, version))
            )
            or paths.validator_dir
        ),
    )
    monkeypatch.setattr(
        mod,
        "run_cargo_command",
        lambda context, command: steps.append(("cargo", (context, list(command)))),
    )

    return _GpuiHarnessPatchState(
        steps=steps,
        workspace_version_args=workspace_version_args,
        packaged_archive_path_args=packaged_archive_path_args,
    )


def test_validate_packaged_gpui_harness_packages_and_tests_artifact(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    run_publish_check_module: ModuleType,
) -> None:
    """Package the GPUI harness, unpack it, and test via a validator crate."""
    workspace = tmp_path / "workspace"
    workspace.mkdir()
    (workspace / "Cargo.toml").write_text("", encoding="utf-8")
    archive = workspace / "target" / "package" / "rstest-bdd-harness-gpui-1.2.3.crate"
    archive.parent.mkdir(parents=True)
    archive.write_text("archive", encoding="utf-8")
    package_dir = workspace / ".gpui-package-check" / "package" / "pkg"
    validator_dir = workspace / ".gpui-package-check" / "validator"

    patch_state = _patch_gpui_harness_functions(
        monkeypatch,
        run_publish_check_module,
        _GpuiPackagePaths(
            archive=archive,
            package_dir=package_dir,
            validator_dir=validator_dir,
        ),
    )
    steps = patch_state.steps

    run_publish_check_module.validate_packaged_gpui_harness(
        "rstest-bdd-harness-gpui",
        workspace,
        timeout_secs=77,
    )

    assert patch_state.workspace_version_args == [workspace / "Cargo.toml"], (
        "expected workspace_version to read the workspace manifest "
        f"from {workspace / 'Cargo.toml'=}, got {patch_state.workspace_version_args=}"
    )
    assert patch_state.packaged_archive_path_args == [
        (workspace, "rstest-bdd-harness-gpui", "1.2.3")
    ], (
        "expected packaged_archive_path to receive workspace, crate, and version "
        f"arguments, got {patch_state.packaged_archive_path_args=}"
    )
    assert len(steps) == 4, (
        f"expected four recorded steps, got {len(steps)=} with {steps=}"
    )
    assert steps[0] == ("archive", (workspace, archive, "1.2.3", 77)), (
        "expected steps[0] to archive "
        f"{workspace=} {archive=} with version 1.2.3 and timeout 77"
    )
    assert steps[1] == (
        "extract",
        (archive, workspace / ".gpui-package-check" / "package"),
    ), f"expected steps[1] to extract {archive=} into {workspace=}"
    assert steps[2] == (
        "validator",
        (
            workspace / ".gpui-package-check" / "validator",
            package_dir,
            workspace / "crates" / "rstest-bdd-harness",
            "1.2.3",
        ),
    ), f"expected steps[2] to write validator with {package_dir=} and {validator_dir=}"
    assert steps[3][0] == "cargo", (
        f"expected steps[3] to record cargo invocation, got {steps[3]=}"
    )
    cargo_context, cargo_command = typ.cast(
        "tuple[run_publish_check_module.CargoCommandContext, list[str]]",
        steps[3][1],
    )
    assert cargo_context.crate == run_publish_check_module.GPUI_VALIDATOR_CRATE, (
        "expected cargo_context.crate to target "
        f"{run_publish_check_module.GPUI_VALIDATOR_CRATE} from {steps=}"
    )
    assert cargo_context.crate_dir == validator_dir, (
        f"expected cargo_context.crate_dir to match {validator_dir=} from {steps=}"
    )
    assert cargo_context.timeout_secs == 77, (
        f"expected cargo_context.timeout_secs to be 77 from {cargo_context=}"
    )
    assert cargo_command == ["cargo", "check", "--tests"], (
        f"expected cargo_command to check tests with {cargo_command=}"
    )


def test_validate_packaged_gpui_harness_rejects_wrong_crate_name(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    run_publish_check_module: ModuleType,
) -> None:
    """Refuse mismatched crate names before constructing a packaged archive."""
    workspace = tmp_path / "workspace"
    workspace.mkdir()

    def fail_if_called(*_args: object, **_kwargs: object) -> typ.NoReturn:
        pytest.fail("packaging helper ran")

    monkeypatch.setattr(
        run_publish_check_module, "build_packaged_archive", fail_if_called
    )
    monkeypatch.setattr(
        run_publish_check_module, "extract_packaged_archive", fail_if_called
    )
    monkeypatch.setattr(
        run_publish_check_module, "write_validator_workspace", fail_if_called
    )

    with pytest.raises(SystemExit, match="validate_packaged_gpui_harness expected"):
        run_publish_check_module.validate_packaged_gpui_harness(
            "not-gpui-harness",
            workspace,
            timeout_secs=77,
        )
