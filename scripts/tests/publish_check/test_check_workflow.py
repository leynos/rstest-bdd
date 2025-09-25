"""Dry-run publish workflow orchestration tests."""

from __future__ import annotations

from pathlib import Path
from types import ModuleType

import pytest


def test_process_crates_for_check_delegates_configuration(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    run_publish_check_module: ModuleType,
) -> None:
    observed: dict[str, object] = {}

    def fake_process_crates(
        workspace: Path,
        timeout_secs: int,
        **kwargs: object,
    ) -> None:
        observed["workspace"] = workspace
        observed["timeout"] = timeout_secs
        observed["kwargs"] = kwargs
        crate_action = kwargs["crate_action"]
        crate_action("rstest-bdd-patterns", workspace, timeout_secs=11)
        crate_action("demo", workspace, timeout_secs=11)

    package_calls: list[tuple[str, Path, int]] = []
    check_calls: list[tuple[str, Path, int]] = []

    monkeypatch.setattr(run_publish_check_module, "_process_crates", fake_process_crates)
    monkeypatch.setattr(
        run_publish_check_module,
        "package_crate",
        lambda crate, root, *, timeout_secs: package_calls.append((crate, root, timeout_secs)),
    )
    monkeypatch.setattr(
        run_publish_check_module,
        "check_crate",
        lambda crate, root, *, timeout_secs: check_calls.append((crate, root, timeout_secs)),
    )

    workspace = tmp_path / "check"
    run_publish_check_module._process_crates_for_check(workspace, 17)

    assert observed["workspace"] == workspace
    assert observed["timeout"] == 17
    kwargs = observed["kwargs"]
    assert kwargs["strip_patch"] is True
    assert kwargs["include_local_path"] is True
    assert kwargs["apply_per_crate"] is False
    assert kwargs.get("per_crate_cleanup") is None
    assert package_calls == [("rstest-bdd-patterns", workspace, 11)]
    assert check_calls == [("demo", workspace, 11)]


def test_process_crates_for_check_runs_local_validation(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    run_publish_check_module: ModuleType,
) -> None:
    workspace = tmp_path / "workspace"
    workspace.mkdir()
    (workspace / "Cargo.toml").write_text("", encoding="utf-8")

    steps: list[tuple[str, object]] = []

    def fake_strip(manifest: Path) -> None:
        steps.append(("strip", manifest))

    monkeypatch.setattr(run_publish_check_module, "strip_patch_section", fake_strip)
    monkeypatch.setattr(run_publish_check_module, "workspace_version", lambda _m: "9.9.9")

    def fake_apply(
        root: Path,
        version: str,
        *,
        include_local_path: bool,
        crates: tuple[str, ...] | None = None,
    ) -> None:
        steps.append(("apply", (root, version, include_local_path, crates)))

    monkeypatch.setattr(run_publish_check_module, "apply_workspace_replacements", fake_apply)

    def fake_package(crate: str, root: Path, *, timeout_secs: int) -> None:
        steps.append(("package", (crate, root, timeout_secs)))

    def fake_check(crate: str, root: Path, *, timeout_secs: int) -> None:
        steps.append(("check", (crate, root, timeout_secs)))

    monkeypatch.setattr(run_publish_check_module, "package_crate", fake_package)
    monkeypatch.setattr(run_publish_check_module, "check_crate", fake_check)
    monkeypatch.setattr(
        run_publish_check_module,
        "CRATE_ORDER",
        ("rstest-bdd-patterns", "crate-b"),
    )

    run_publish_check_module._process_crates_for_check(workspace, 55)

    manifest = workspace / "Cargo.toml"
    assert steps == [
        ("strip", manifest),
        ("apply", (workspace, "9.9.9", True, None)),
        ("package", ("rstest-bdd-patterns", workspace, 55)),
        ("check", ("crate-b", workspace, 55)),
    ]
