#!/usr/bin/env -S uv run python
"""Helpers for validating the packaged GPUI harness against upstream GPUI.

The main publish workflow keeps the repository on a stable-compatible GPUI shim
via a workspace patch. This module creates an isolated validator crate that
depends on the packaged `rstest-bdd-harness-gpui` artifact and the upstream
`gpui` crate from crates.io, allowing release automation to verify that the
published dependency graph still works without changing the main workspace.
"""

from __future__ import annotations

import pathlib
import shutil
import tarfile
import typing as typ

import tomllib

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
    """Create a standalone publish-shaped archive for the GPUI harness crate.

    Examples
    --------
    >>> build_packaged_archive(  # doctest: +SKIP
    ...     Path('/tmp/workspace'),
    ...     Path('/tmp/workspace/target/package/rstest-bdd-harness-gpui-1.2.3.crate'),
    ...     '1.2.3',
    ... )
    PosixPath('/tmp/workspace/target/package/rstest-bdd-harness-gpui-1.2.3.crate')
    """
    source_dir = workspace_root / "crates" / GPUI_HARNESS_CRATE
    package_root = destination.parent / f"{GPUI_HARNESS_CRATE}-{version}"
    if package_root.exists():
        shutil.rmtree(package_root)
    package_root.mkdir(parents=True, exist_ok=True)

    shutil.copytree(source_dir / "src", package_root / "src")
    shutil.copy2(source_dir / "README.md", package_root / "README.md")
    (package_root / "Cargo.toml").write_text(
        _packaged_manifest(workspace_root, version),
        encoding="utf-8",
    )

    destination.parent.mkdir(parents=True, exist_ok=True)
    with tarfile.open(destination, "w:gz") as package:
        for path in sorted(package_root.rglob("*")):
            if not path.is_file():
                continue
            relative = path.relative_to(package_root)
            package.add(path, arcname=f"{package_root.name}/{relative.as_posix()}")

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


def _packaged_manifest(workspace_root: Path, version: str) -> str:
    workspace = tomllib.loads(
        (workspace_root / "Cargo.toml").read_text(encoding="utf-8")
    )
    crate = tomllib.loads(
        (workspace_root / "crates" / GPUI_HARNESS_CRATE / "Cargo.toml").read_text(
            encoding="utf-8"
        )
    )
    workspace_package = workspace["workspace"]["package"]
    package = crate["package"]

    return """[package]
name = "{name}"
version = "{version}"
edition = "{edition}"
license = "{license}"
authors = {authors}
description = "{description}"
homepage = "{homepage}"
repository = "{repository}"
readme = "{readme}"
keywords = {keywords}
categories = {categories}
rust-version = "{rust_version}"

[lib]
doctest = false
test = false

[features]
native-gpui-tests = []

[dependencies]
rstest-bdd-harness = "{version}"
gpui = {{ version = "0.2.2", default-features = false, features = ["test-support"] }}
""".format(
        name=package["name"],
        version=version,
        edition=workspace_package["edition"],
        license=workspace_package["license"],
        authors=_toml_list(workspace_package["authors"]),
        description=package["description"],
        homepage=workspace_package["homepage"],
        repository=workspace_package["repository"],
        readme=package["readme"],
        keywords=_toml_list(workspace_package["keywords"]),
        categories=_toml_list(workspace_package["categories"]),
        rust_version=workspace_package["rust-version"],
    )


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


def _toml_list(values: list[str]) -> str:
    quoted = ", ".join(f'"{value}"' for value in values)
    return f"[{quoted}]"


def _extract_archive_safely(package: tarfile.TarFile, destination: Path) -> None:
    resolved_destination = pathlib.Path(destination).resolve(strict=False)
    for member in package.getmembers():
        if _is_unsafe_archive_path(resolved_destination, member.name):
            message = f"refusing to extract unsafe archive member {member.name!r}"
            raise SystemExit(message)
        member_destination = _archive_target_path(resolved_destination, member.name)
        if member_destination is None:
            message = f"refusing to extract unsafe archive member {member.name!r}"
            raise SystemExit(message)
        if (member.issym() or member.islnk()) and _is_unsafe_archive_path(
            resolved_destination,
            member.linkname,
        ):
            message = f"refusing to extract unsafe archive member {member.name!r}"
            raise SystemExit(message)
        package.extract(member, destination)


def _is_unsafe_archive_path(
    destination: pathlib.Path,
    path_name: str,
    *,
    base_directory: pathlib.Path | None = None,
) -> bool:
    target = _archive_target_path(base_directory or destination, path_name)
    return target is None or not _is_within_directory(destination, target)


def _archive_target_path(
    base_directory: pathlib.Path, path_name: str
) -> pathlib.Path | None:
    posix_path = pathlib.PurePosixPath(path_name.replace("\\", "/"))
    windows_path = pathlib.PureWindowsPath(path_name)
    if (
        posix_path.is_absolute()
        or windows_path.is_absolute()
        or bool(windows_path.drive)
    ):
        return None
    relative_parts = [part for part in posix_path.parts if part not in ("", ".")]
    return base_directory.joinpath(*relative_parts).resolve(strict=False)


def _is_within_directory(root: pathlib.Path, target: pathlib.Path) -> bool:
    return target == root or root in target.parents
