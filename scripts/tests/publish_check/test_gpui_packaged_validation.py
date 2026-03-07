"""Tests for the packaged GPUI harness validation helpers."""

from __future__ import annotations

import tarfile
import typing as typ

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

    assert archive == tmp_path / "target" / "package" / "demo-1.2.3.crate"


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
    assert 'version = "1.2.3"' in manifest
    assert 'rstest-bdd-harness = "1.2.3"' in manifest
    expected_gpui_dependency = (
        'gpui = { version = "0.2.2", default-features = false, '
        'features = ["test-support"] }'
    )
    assert expected_gpui_dependency in manifest


def test_extract_packaged_archive_returns_package_root(tmp_path: Path) -> None:
    """Extract the archive and return the top-level packaged directory."""
    archive = tmp_path / "demo-1.2.3.crate"
    with tarfile.open(archive, "w:gz") as package:
        source = tmp_path / "source.txt"
        source.write_text("hello", encoding="utf-8")
        package.add(source, arcname="demo-1.2.3/Cargo.toml")

    extracted = extract_packaged_archive(archive, tmp_path / "out")

    assert extracted == tmp_path / "out" / "demo-1.2.3"
    assert (extracted / "Cargo.toml").read_text(encoding="utf-8") == "hello"


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

    assert validator == tmp_path / "validator"
    assert f'name = "{GPUI_VALIDATOR_CRATE}"' in manifest
    assert 'rstest-bdd-harness = "1.2.3"' in manifest
    assert package_dir.as_posix() in manifest
    assert harness_dir.as_posix() in manifest
    assert "packaged_gpui_harness_runs_against_upstream_gpui" in test_source
    assert "#[gpui::test]" in test_source
