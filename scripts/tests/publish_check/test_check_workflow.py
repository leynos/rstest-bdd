"""Unit tests covering the publish-check validation workflow."""

from __future__ import annotations

import typing as typ

if typ.TYPE_CHECKING:
    from pathlib import Path
    from types import ModuleType

    import pytest


def test_process_crates_for_check_delegates_configuration(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
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

    package_calls: list[tuple[str, Path, int]] = []
    gpui_calls: list[tuple[str, Path, int]] = []
    check_calls: list[tuple[str, Path, int]] = []

    monkeypatch.setattr(
        run_publish_check_module, "_process_crates", fake_process_crates
    )
    monkeypatch.setattr(
        run_publish_check_module,
        "package_crate",
        lambda crate, root, *, timeout_secs: package_calls.append(
            (crate, root, timeout_secs)
        ),
    )
    monkeypatch.setattr(
        run_publish_check_module,
        "check_crate",
        lambda crate, root, *, timeout_secs: check_calls.append(
            (crate, root, timeout_secs)
        ),
    )
    monkeypatch.setattr(
        run_publish_check_module,
        "validate_packaged_gpui_harness",
        lambda crate, root, *, timeout_secs: gpui_calls.append(
            (crate, root, timeout_secs)
        ),
    )

    workspace = tmp_path / "check"
    run_publish_check_module._process_crates_for_check(workspace, 17)

    assert observed["workspace"] == workspace
    assert observed["timeout"] == 17
    config = observed["config"]
    assert isinstance(config, run_publish_check_module.CrateProcessingConfig)
    assert config.strip_patch is True
    assert config.include_local_path is True
    assert config.apply_per_crate is False
    assert config.per_crate_cleanup is None
    assert callable(observed["crate_action"])
    assert package_calls == [("rstest-bdd-patterns", workspace, 11)]
    assert gpui_calls == [("rstest-bdd-harness-gpui", workspace, 11)]
    assert check_calls == [("demo", workspace, 11)]


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
    ]


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

    steps: list[tuple[str, object]] = []

    monkeypatch.setattr(
        run_publish_check_module, "workspace_version", lambda _manifest: "1.2.3"
    )
    monkeypatch.setattr(
        run_publish_check_module,
        "build_packaged_archive",
        lambda root, archive_path, version: steps.append(
            ("archive", (root, archive_path, version))
        ),
    )
    monkeypatch.setattr(
        run_publish_check_module,
        "packaged_archive_path",
        lambda root, crate, version: archive,
    )
    monkeypatch.setattr(
        run_publish_check_module,
        "extract_packaged_archive",
        lambda archive_path, destination: (
            steps.append(("extract", (archive_path, destination))) or package_dir
        ),
    )
    monkeypatch.setattr(
        run_publish_check_module,
        "write_validator_workspace",
        lambda destination, *, package_dir, harness_dir, version: (
            steps.append(
                ("validator", (destination, package_dir, harness_dir, version))
            )
            or validator_dir
        ),
    )
    monkeypatch.setattr(
        run_publish_check_module,
        "run_cargo_command",
        lambda context, command: steps.append(("cargo", (context, list(command)))),
    )

    run_publish_check_module.validate_packaged_gpui_harness(
        "rstest-bdd-harness-gpui",
        workspace,
        timeout_secs=77,
    )

    assert steps[0] == ("archive", (workspace, archive, "1.2.3"))
    assert steps[1] == (
        "extract",
        (archive, workspace / ".gpui-package-check" / "package"),
    )
    assert steps[2] == (
        "validator",
        (
            workspace / ".gpui-package-check" / "validator",
            package_dir,
            workspace / "crates" / "rstest-bdd-harness",
            "1.2.3",
        ),
    )
    cargo_context, cargo_command = typ.cast(
        "tuple[run_publish_check_module.CargoCommandContext, list[str]]",
        steps[3][1],
    )
    assert cargo_context.crate == run_publish_check_module.GPUI_VALIDATOR_CRATE
    assert cargo_context.crate_dir == validator_dir
    assert cargo_context.timeout_secs == 77
    assert cargo_command == ["cargo", "check", "--tests"]
