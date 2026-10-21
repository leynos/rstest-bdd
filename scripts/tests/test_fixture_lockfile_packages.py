"""Unit tests for reading resolved packages out of a lockfile or manifest.

The alignment gate is only as good as its reading: a lockfile that fails to
parse must fail the gate rather than compare as empty, and a fixture's own
crate must be identifiable from its manifest. Both are pinned here.
"""

import tomllib
from pathlib import Path

import pytest
from fixture_lockfile_packages import (
    LockfileUnreadableError,
    own_package,
    read_lockfile_packages,
)

REPO_ROOT = Path(__file__).resolve().parents[2]

CRATES_IO = "registry+https://github.com/rust-lang/crates.io-index"


def test_read_lockfile_ignores_entries_without_a_version() -> None:
    """A malformed entry cannot become a phantom package."""
    lockfile = REPO_ROOT / "target" / "alignment-fixture-ignore.lock"
    lockfile.parent.mkdir(parents=True, exist_ok=True)
    lockfile.write_text(
        '[[package]]\nname = "no-version"\n[[package]]\n'
        'name = "kept"\nversion = "1.0.0"\n',
        encoding="utf-8",
    )
    try:
        assert read_lockfile_packages(lockfile) == {("kept", "1.0.0", None)}, (
            "an entry without a version must not become a phantom package"
        )
    finally:
        lockfile.unlink()


def test_read_lockfile_records_the_source_of_each_package() -> None:
    """A registry package keeps its source; a path package has none."""
    lockfile = REPO_ROOT / "target" / "alignment-fixture-sources.lock"
    lockfile.parent.mkdir(parents=True, exist_ok=True)
    lockfile.write_text(
        '[[package]]\nname = "serde"\nversion = "1.0.228"\n'
        f'source = "{CRATES_IO}"\n'
        '[[package]]\nname = "fixture"\nversion = "0.1.0"\n',
        encoding="utf-8",
    )
    try:
        assert read_lockfile_packages(lockfile) == {
            ("serde", "1.0.228", CRATES_IO),
            ("fixture", "0.1.0", None),
        }, "a package's source is part of its identity"
    finally:
        lockfile.unlink()


def test_read_lockfile_rejects_invalid_toml(tmp_path: Path) -> None:
    """An unreadable lockfile fails the gate instead of comparing as empty."""
    lockfile = tmp_path / "Cargo.lock"
    lockfile.write_text("this is not toml = = =\n", encoding="utf-8")
    with pytest.raises(LockfileUnreadableError):
        read_lockfile_packages(lockfile)


def test_read_lockfile_rejects_a_missing_file(tmp_path: Path) -> None:
    """A missing lockfile is an error, not an empty graph."""
    with pytest.raises(LockfileUnreadableError):
        read_lockfile_packages(tmp_path / "absent.lock")


def test_own_package_reads_the_manifest_package_table(tmp_path: Path) -> None:
    """The fixture's own crate is identified from its manifest."""
    manifest = tmp_path / "Cargo.toml"
    manifest.write_text(
        '[package]\nname = "my-fixture"\nversion = "0.1.0"\n[workspace]\n',
        encoding="utf-8",
    )
    assert own_package(manifest) == ("my-fixture", "0.1.0", None), (
        "the fixture's own package must be read from its manifest"
    )


def test_own_package_is_none_without_a_package_table(tmp_path: Path) -> None:
    """A virtual manifest owns no package, so nothing is exempted."""
    manifest = tmp_path / "Cargo.toml"
    manifest.write_text("[workspace]\n", encoding="utf-8")
    assert own_package(manifest) is None, (
        "a virtual manifest must not exempt anything from the comparison"
    )


def test_own_package_ignores_an_unreadable_package_table(tmp_path: Path) -> None:
    """A manifest whose ``package`` is not a table owns no package."""
    manifest = tmp_path / "Cargo.toml"
    manifest.write_text('package = "not-a-table"\n', encoding="utf-8")
    assert own_package(manifest) is None, (
        "a malformed package table must not exempt an arbitrary name"
    )


def test_the_repository_lockfile_parses() -> None:
    """The real root lockfile is readable, so the gate has a workspace graph."""
    packages = read_lockfile_packages(REPO_ROOT / "Cargo.lock")
    assert packages, "the root lockfile must resolve at least one package"
    assert any(source == CRATES_IO for _, _, source in packages), (
        "the workspace must resolve at least one crate from crates.io"
    )


def test_the_reading_matches_what_tomllib_reports() -> None:
    """The reading loses nothing: every well-formed entry is present.

    A guard against the identity function silently dropping the entries whose
    source is absent, which is the whole path-dependency half of the graph.
    """
    lockfile = REPO_ROOT / "Cargo.lock"
    document = tomllib.loads(lockfile.read_text(encoding="utf-8"))
    expected = {
        (entry["name"], entry["version"], entry.get("source"))
        for entry in document["package"]
    }
    assert read_lockfile_packages(lockfile) == expected, (
        "the reading must report exactly the entries the lockfile holds"
    )
