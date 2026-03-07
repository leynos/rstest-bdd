#!/usr/bin/env -S uv run python
"""Helpers for validating the packaged GPUI harness against upstream GPUI.

The main publish workflow keeps the repository on a stable-compatible GPUI shim
via a workspace patch. This module creates an isolated validator crate that
depends on the packaged `rstest-bdd-harness-gpui` artifact and the upstream
`gpui` crate from crates.io, allowing release automation to verify that the
published dependency graph still works without changing the main workspace.
"""

from __future__ import annotations

import subprocess
import tarfile
import typing as typ

if typ.TYPE_CHECKING:
    from pathlib import Path

GPUI_HARNESS_CRATE = "rstest-bdd-harness-gpui"
GPUI_VALIDATOR_CRATE = "rstest-bdd-harness-gpui-package-check"


def packaged_archive_path(workspace_root: Path, crate: str, version: str) -> Path:
    """Return the expected archive path produced by ``cargo package``.

    Examples
    --------
    >>> packaged_archive_path(Path("/tmp/workspace"), "demo", "1.2.3")
    PosixPath('/tmp/workspace/target/package/demo-1.2.3.crate')
    """
    return workspace_root / "target" / "package" / f"{crate}-{version}.crate"


def build_packaged_archive(
    workspace_root: Path, destination: Path, version: str
) -> Path:
    """Ask Cargo to package the GPUI harness crate into ``destination``.

    Examples
    --------
    >>> build_packaged_archive(  # doctest: +SKIP
    ...     Path('/tmp/workspace'),
    ...     Path('/tmp/workspace/target/package/rstest-bdd-harness-gpui-1.2.3.crate'),
    ...     '1.2.3',
    ... )
    PosixPath('/tmp/workspace/target/package/rstest-bdd-harness-gpui-1.2.3.crate')
    """
    crate_manifest = workspace_root / "crates" / GPUI_HARNESS_CRATE / "Cargo.toml"
    destination.parent.mkdir(parents=True, exist_ok=True)
    command = [
        "cargo",
        "package",
        "--manifest-path",
        str(crate_manifest),
        "--allow-dirty",
        "--no-verify",
    ]
    completed = subprocess.run(  # noqa: S603 - fixed cargo invocation
        command,
        check=False,
        cwd=workspace_root,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        message = completed.stderr.strip() or completed.stdout.strip()
        error_message = f"cargo package failed for {GPUI_HARNESS_CRATE}: {message}"
        raise SystemExit(error_message)
    if not destination.exists():
        message = f"cargo package did not produce expected archive {destination}"
        raise SystemExit(message)

    return destination


def extract_packaged_archive(archive: Path, destination: Path) -> Path:
    """Extract ``archive`` into ``destination`` and return the package root.

    Examples
    --------
    >>> archive = Path("/tmp/workspace/target/package/demo-1.2.3.crate")
    >>> destination = Path("/tmp/unpacked")
    >>> extract_packaged_archive(archive, destination)  # doctest: +SKIP
    PosixPath('/tmp/unpacked/demo-1.2.3')
    """
    destination.mkdir(parents=True, exist_ok=True)
    with tarfile.open(archive, "r:gz") as package:
        _extract_archive_safely(package, destination)
        members = [member.name for member in package.getmembers() if member.name]

    try:
        package_root_name = min(
            member.split("/", 1)[0] for member in members if not member.startswith("/")
        )
    except ValueError as error:
        message = f"packaged archive {archive} did not contain any files"
        raise SystemExit(message) from error

    return destination / package_root_name


def write_validator_workspace(
    destination: Path,
    *,
    package_dir: Path,
    harness_dir: Path,
    version: str,
) -> Path:
    """Create a minimal validator crate that targets upstream ``gpui``.

    Examples
    --------
    >>> write_validator_workspace(  # doctest: +SKIP
    ...     Path('/tmp/validator'),
    ...     package_dir=Path('/tmp/pkg/rstest-bdd-harness-gpui-1.2.3'),
    ...     harness_dir=Path('/tmp/workspace/crates/rstest-bdd-harness'),
    ...     version='1.2.3',
    ... )
    PosixPath('/tmp/validator')
    """
    destination.mkdir(parents=True, exist_ok=True)
    tests_dir = destination / "tests"
    tests_dir.mkdir(exist_ok=True)
    (destination / "Cargo.toml").write_text(
        _validator_manifest(
            package_dir=package_dir,
            harness_dir=harness_dir,
            version=version,
        ),
        encoding="utf-8",
    )
    (tests_dir / "packaged_gpui_harness.rs").write_text(
        _validator_test_source(),
        encoding="utf-8",
    )
    return destination


def _validator_manifest(*, package_dir: Path, harness_dir: Path, version: str) -> str:
    package_path = _toml_path(package_dir)
    harness_path = _toml_path(harness_dir)
    return f"""[package]
name = "{GPUI_VALIDATOR_CRATE}"
version = "0.0.0"
edition = "2024"
publish = false

[workspace]

[dependencies]
gpui = {{ version = "0.2.2", default-features = false, features = ["test-support"] }}
rstest-bdd-harness = "{version}"
rstest-bdd-harness-gpui = {{ path = "{package_path}" }}

[patch.crates-io]
rstest-bdd-harness = {{ path = "{harness_path}" }}
"""


def _validator_test_source() -> str:
    return """//! Smoke tests for the packaged GPUI harness artifact.

use rstest_bdd_harness::{
    HarnessAdapter, ScenarioMetadata, ScenarioRunRequest, ScenarioRunner,
};
use rstest_bdd_harness_gpui::GpuiHarness;

#[test]
fn packaged_gpui_harness_runs_against_upstream_gpui() {
    let request = ScenarioRunRequest::new(
        ScenarioMetadata::new(
            "tests/features/demo.feature",
            "Packaged GPUI harness",
            7,
            vec!["@ui".to_string()],
        ),
        ScenarioRunner::new(|context: gpui::TestAppContext| {
            context.test_function_name().is_none()
        }),
    );

    assert!(GpuiHarness::new().run(request));
}

#[gpui::test]
fn upstream_gpui_attribute_runs(context: &gpui::TestAppContext) {
    assert_eq!(context.test_function_name(), Some("upstream_gpui_attribute_runs"));
}
"""


def _toml_path(path: Path) -> str:
    return path.as_posix().replace("\\", "\\\\")


def _extract_archive_safely(package: tarfile.TarFile, destination: Path) -> None:
    for member in package.getmembers():
        if _is_unsafe_archive_path(member.name):
            message = f"refusing to extract unsafe archive member {member.name!r}"
            raise SystemExit(message)
        if (member.issym() or member.islnk()) and _is_unsafe_archive_path(
            member.linkname
        ):
            message = f"refusing to extract unsafe archive member {member.name!r}"
            raise SystemExit(message)
        package.extract(member, destination)


def _is_unsafe_archive_path(path_name: str) -> bool:
    path_parts = path_name.split("/")
    return path_name.startswith("/") or any(part == ".." for part in path_parts)
