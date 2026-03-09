"""Tests for the packaged GPUI harness validation helpers."""

from __future__ import annotations

import tarfile
import typing as typ

import pytest
from publish_check_gpui import (
    GPUI_VALIDATOR_CRATE,
    build_packaged_archive,
    extract_packaged_archive,
    packaged_archive_path,
    write_validator_workspace,
)

if typ.TYPE_CHECKING:
    from pathlib import Path


def test_packaged_archive_path_targets_cargo_package_output(tmp_path: Path) -> None:
    """Resolve archives under ``target/package`` with the Cargo crate suffix."""
    archive = packaged_archive_path(tmp_path, "demo", "1.2.3")

    assert archive == tmp_path / "target" / "package" / "demo-1.2.3.crate", (
        "expected packaged crate path"
    )


def test_build_packaged_archive_creates_standalone_gpui_harness_archive(
    tmp_path: Path,
) -> None:
    """Build a publish-shaped archive with an explicit standalone manifest."""
    workspace = tmp_path / "workspace"
    crate_dir = workspace / "crates" / "rstest-bdd-harness-gpui"
    (crate_dir / "src").mkdir(parents=True)
    (crate_dir / "src" / "lib.rs").write_text("// test", encoding="utf-8")
    (crate_dir / "README.md").write_text("# demo", encoding="utf-8")
    (crate_dir / "Cargo.toml").write_text(
        "\n".join(
            (
                "[package]",
                'name = "rstest-bdd-harness-gpui"',
                'description = "demo"',
                'readme = "README.md"',
            )
        ),
        encoding="utf-8",
    )
    (workspace / "Cargo.toml").write_text(
        "\n".join(
            (
                "[workspace.package]",
                'edition = "2024"',
                'license = "ISC"',
                'authors = ["Tester <test@example.com>"]',
                'homepage = "https://example.invalid"',
                'repository = "https://example.invalid/repo"',
                'keywords = ["bdd"]',
                'categories = ["development-tools::testing"]',
                'rust-version = "1.85"',
            )
        ),
        encoding="utf-8",
    )
    archive = packaged_archive_path(workspace, "rstest-bdd-harness-gpui", "1.2.3")

    build_packaged_archive(workspace, archive, "1.2.3")

    extracted = extract_packaged_archive(archive, tmp_path / "out")
    manifest = (extracted / "Cargo.toml").read_text(encoding="utf-8")
    assert 'version = "1.2.3"' in manifest, "expected packaged version in manifest"
    assert 'rstest-bdd-harness = "1.2.3"' in manifest, (
        "expected harness dependency version in manifest"
    )
    expected_gpui_dependency = (
        'gpui = { version = "0.2.2", default-features = false, '
        'features = ["test-support"] }'
    )
    assert expected_gpui_dependency in manifest, (
        "expected upstream gpui dependency in manifest"
    )


def test_extract_packaged_archive_returns_package_root(tmp_path: Path) -> None:
    """Extract the archive and return the top-level packaged directory."""
    archive = tmp_path / "demo-1.2.3.crate"
    with tarfile.open(archive, "w:gz") as package:
        source = tmp_path / "source.txt"
        source.write_text("hello", encoding="utf-8")
        package.add(source, arcname="demo-1.2.3/Cargo.toml")

    extracted = extract_packaged_archive(archive, tmp_path / "out")

    assert extracted == tmp_path / "out" / "demo-1.2.3", (
        "expected extracted package root"
    )
    assert (extracted / "Cargo.toml").read_text(encoding="utf-8") == "hello", (
        "expected extracted manifest contents"
    )


def test_extract_packaged_archive_rejects_multiple_top_level_directories(
    tmp_path: Path,
) -> None:
    """Reject archives that contain more than one top-level directory."""
    archive = tmp_path / "demo-1.2.3.crate"
    with tarfile.open(archive, "w:gz") as package:
        first = tmp_path / "first.txt"
        second = tmp_path / "second.txt"
        first.write_text("first", encoding="utf-8")
        second.write_text("second", encoding="utf-8")
        package.add(first, arcname="demo-1.2.3/Cargo.toml")
        package.add(second, arcname="other-1.2.3/README.md")

    with pytest.raises(
        SystemExit,
        match="must contain exactly one top-level directory",
    ):
        extract_packaged_archive(archive, tmp_path / "out")


def test_extract_packaged_archive_rejects_top_level_file_entries(
    tmp_path: Path,
) -> None:
    """Reject archives that place files directly at the archive root."""
    archive = tmp_path / "demo-1.2.3.crate"
    with tarfile.open(archive, "w:gz") as package:
        source = tmp_path / "source.txt"
        source.write_text("hello", encoding="utf-8")
        package.add(source, arcname="Cargo.toml")

    with pytest.raises(SystemExit, match="contained top-level file entries"):
        extract_packaged_archive(archive, tmp_path / "out")


def test_extract_packaged_archive_rejects_unsafe_symlink_target(tmp_path: Path) -> None:
    """Reject symlink members whose link target escapes the destination."""
    archive = tmp_path / "demo-1.2.3.crate"
    with tarfile.open(archive, "w:gz") as package:
        symlink = tarfile.TarInfo("demo-1.2.3/link")
        symlink.type = tarfile.SYMTYPE
        symlink.linkname = "../outside"
        package.addfile(symlink)

    with pytest.raises(SystemExit, match="refusing to extract unsafe archive member"):
        extract_packaged_archive(archive, tmp_path / "out")


def test_extract_packaged_archive_rejects_windows_style_escape(tmp_path: Path) -> None:
    """Reject backslash-separated paths that would escape on Windows."""
    archive = tmp_path / "demo-1.2.3.crate"
    with tarfile.open(archive, "w:gz") as package:
        source = tmp_path / "source.txt"
        source.write_text("hello", encoding="utf-8")
        package.add(source, arcname="..\\outside\\Cargo.toml")

    with pytest.raises(SystemExit, match="refusing to extract unsafe archive member"):
        extract_packaged_archive(archive, tmp_path / "out")


def test_write_validator_workspace_writes_manifest_and_smoke_test(
    tmp_path: Path,
) -> None:
    """Generate a validator crate that points at the packaged harness artifact."""
    package_dir = tmp_path / "pkg" / "rstest-bdd-harness-gpui-1.2.3"
    harness_dir = tmp_path / "workspace" / "crates" / "rstest-bdd-harness"

    validator = write_validator_workspace(
        tmp_path / "validator",
        package_dir=package_dir,
        harness_dir=harness_dir,
        version="1.2.3",
    )

    manifest = (validator / "Cargo.toml").read_text(encoding="utf-8")
    test_source = (validator / "tests" / "packaged_gpui_harness.rs").read_text(
        encoding="utf-8"
    )

    assert validator == tmp_path / "validator", "expected validator workspace path"
    assert f'name = "{GPUI_VALIDATOR_CRATE}"' in manifest, (
        "expected validator crate name in manifest"
    )
    assert 'rstest-bdd-harness = "1.2.3"' in manifest, (
        "expected harness dependency version in validator manifest"
    )
    assert package_dir.as_posix() in manifest, (
        "expected packaged harness path in validator manifest"
    )
    assert harness_dir.as_posix() in manifest, (
        "expected local harness patch path in validator manifest"
    )
    assert "packaged_gpui_harness_runs_against_upstream_gpui" in test_source, (
        "expected smoke test function in validator source"
    )
    assert "#[gpui::test]" in test_source, (
        "expected gpui test attribute in validator source"
    )
