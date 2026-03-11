"""Unit tests covering the publish-check validation workflow."""

from __future__ import annotations

import dataclasses
import typing as typ

import pytest

from .conftest import CrateActionCalls, GpuiHarnessPatchState, GpuiPackagePaths

if typ.TYPE_CHECKING:
    from pathlib import Path
    from types import ModuleType

    class _CargoContextLike(typ.Protocol):
        crate: str
        crate_dir: Path
        timeout_secs: int


def test_process_crates_for_check_delegates_configuration(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    crate_action_calls: CrateActionCalls,
    run_publish_check_module: ModuleType,
) -> None:
    """Ensure the check flow supplies the expected processing configuration."""
    observed: dict[str, object] = {}

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
    match config:
        case run_publish_check_module.CrateProcessingConfig(
            strip_patch=True,
            include_local_path=True,
            apply_per_crate=False,
            per_crate_cleanup=None,
        ):
            pass
        case _:
            message = f"expected stripped local-path shared config, got {config=!r}"
            raise AssertionError(message)
    assert callable(observed["crate_action"]), (
        f"expected callable crate_action, got {observed['crate_action']=}"
    )
    assert crate_action_calls.package == [("rstest-bdd-patterns", workspace, 11)], (
        "expected crate_action_calls.package for patterns crate, "
        f"got {crate_action_calls.package=}"
    )
    assert crate_action_calls.gpui == [("rstest-bdd-harness-gpui", workspace, 11)], (
        "expected crate_action_calls.gpui for GPUI harness, "
        f"got {crate_action_calls.gpui=}"
    )
    assert crate_action_calls.check == [("demo", workspace, 11)], (
        "expected crate_action_calls.check for demo crate, "
        f"got {crate_action_calls.check=}"
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
class _GpuiHarnessTestContext:
    workspace: Path
    package_dir: Path
    validator_dir: Path
    timeout_secs: int


def _assert_gpui_harness_artifact_steps(
    patch_state: GpuiHarnessPatchState,
    ctx: _GpuiHarnessTestContext,
    mod: ModuleType,
) -> None:
    """Assert the full step sequence recorded by the gpui_harness_calls fixture."""
    archive = (
        ctx.workspace / "target" / "package" / "rstest-bdd-harness-gpui-1.2.3.crate"
    )
    steps = patch_state.steps

    assert patch_state.workspace_version_args == [ctx.workspace / "Cargo.toml"], (
        "expected workspace_version to read the workspace manifest "
        f"from {ctx.workspace / 'Cargo.toml'=}, got "
        f"{patch_state.workspace_version_args=}"
    )
    assert patch_state.packaged_archive_path_args == [
        (ctx.workspace, "rstest-bdd-harness-gpui", "1.2.3")
    ], (
        "expected packaged_archive_path to receive workspace, crate, and version "
        f"arguments, got {patch_state.packaged_archive_path_args=}"
    )
    assert len(steps) == 4, (
        f"expected four recorded steps, got {len(steps)=} with {steps=}"
    )
    assert steps[0] == (
        "archive",
        (ctx.workspace, archive, "1.2.3", ctx.timeout_secs),
    ), (
        "expected steps[0] to archive "
        f"{ctx.workspace=} with version 1.2.3 and timeout "
        f"{ctx.timeout_secs}"
    )
    assert steps[1] == (
        "extract",
        (archive, ctx.workspace / ".gpui-package-check" / "package"),
    ), f"expected steps[1] to extract archive into {ctx.workspace=}"
    assert steps[2] == (
        "validator",
        (
            ctx.workspace / ".gpui-package-check" / "validator",
            ctx.package_dir,
            ctx.workspace / "crates" / "rstest-bdd-harness",
            "1.2.3",
        ),
    ), (
        "expected steps[2] to write validator with "
        f"{ctx.package_dir=} and {ctx.validator_dir=}"
    )
    assert steps[3][0] == "cargo", (
        f"expected steps[3] to record cargo invocation, got {steps[3]=}"
    )
    cargo_context, cargo_command = typ.cast(
        "tuple[_CargoContextLike, list[str]]",
        steps[3][1],
    )
    assert cargo_context.crate == mod.GPUI_VALIDATOR_CRATE, (
        "expected cargo_context.crate to target "
        f"{mod.GPUI_VALIDATOR_CRATE} from {steps=}"
    )
    assert cargo_context.crate_dir == ctx.validator_dir, (
        f"expected cargo_context.crate_dir to match {ctx.validator_dir=} from {steps=}"
    )
    assert cargo_context.timeout_secs == ctx.timeout_secs, (
        "expected cargo_context.timeout_secs to be "
        f"{ctx.timeout_secs} from {cargo_context=}"
    )
    assert cargo_command == ["cargo", "check", "--tests"], (
        f"expected cargo_command to check tests with {cargo_command=}"
    )


def test_validate_packaged_gpui_harness_packages_and_tests_artifact(
    gpui_harness_calls: typ.Callable[[GpuiPackagePaths], GpuiHarnessPatchState],
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

    patch_state = gpui_harness_calls(
        GpuiPackagePaths(
            archive=archive,
            package_dir=package_dir,
            validator_dir=validator_dir,
        )
    )

    run_publish_check_module.validate_packaged_gpui_harness(
        "rstest-bdd-harness-gpui",
        workspace,
        timeout_secs=77,
    )

    _assert_gpui_harness_artifact_steps(
        patch_state,
        _GpuiHarnessTestContext(
            workspace=workspace,
            package_dir=package_dir,
            validator_dir=validator_dir,
            timeout_secs=77,
        ),
        run_publish_check_module,
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
        run_publish_check_module, "packaged_archive_path", fail_if_called
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
