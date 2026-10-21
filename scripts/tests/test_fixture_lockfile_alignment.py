"""Unit tests for the fixture alignment gate.

The gate's value is entirely in how it fails: a check that cannot report drift,
or that reports a fixture's own crate as drift, would either pass over the
regression it exists to catch or cry wolf on every run. These tests pin the
comparison, the exemption rules, and the reports.
"""

import tomllib
from pathlib import Path

import pytest
from fixture_lockfile_alignment import (
    EXEMPT_FIXTURES,
    alignment_failure,
    alignment_failures,
    check_alignment,
    divergent_packages,
    format_package,
    is_exempt,
)

REPO_ROOT = Path(__file__).resolve().parents[2]

CRATES_IO = "registry+https://github.com/rust-lang/crates.io-index"


def _write_lockfile(path: Path, packages: list[tuple[str, str, str | None]]) -> None:
    """Write a minimal lockfile holding *packages*."""
    entries = []
    for name, version, source in packages:
        entry = f'[[package]]\nname = "{name}"\nversion = "{version}"\n'
        if source is not None:
            entry += f'source = "{source}"\n'
        entries.append(entry)
    path.write_text("\n".join(entries), encoding="utf-8")


def test_package_identity_distinguishes_sources() -> None:
    """Same name and version from different sources are different packages."""
    registry = ("serde", "1.0.228", CRATES_IO)
    patched = ("serde", "1.0.228", None)
    assert divergent_packages({registry}, {patched}) == [patched], (
        "a path override of a registry crate is a different package"
    )


def test_divergent_packages_reports_only_the_difference() -> None:
    """A shared package is not drift, and the report is sorted."""
    shared = ("serde", "1.0.228", CRATES_IO)
    extra_a = ("aaa", "1.0.0", CRATES_IO)
    extra_z = ("zzz", "1.0.0", CRATES_IO)
    assert divergent_packages({shared}, {shared, extra_z, extra_a}) == [
        extra_a,
        extra_z,
    ], "a shared package must not be drift, and the report must be sorted"


def test_format_package_omits_absent_source() -> None:
    """A path package renders without an empty source suffix."""
    assert format_package(("fixture", "0.1.0", None)) == "fixture 0.1.0", (
        "a path package must render as name and version only"
    )
    assert format_package(("syn", "3.0.5", CRATES_IO)) == f"syn 3.0.5 ({CRATES_IO})", (
        "a registry package must name its source"
    )


def test_alignment_failure_names_every_divergent_package() -> None:
    """The report lists each package so the fix needs no further lookup."""
    report = alignment_failure(
        Path("crates/example/Cargo.toml"), [("dashmap", "5.5.3", CRATES_IO)]
    )
    assert "crates/example/Cargo.toml" in report, "the report must name the fixture"
    assert "1 package(s)" in report, "the report must count the divergent packages"
    assert f"dashmap 5.5.3 ({CRATES_IO})" in report, (
        "the report must name each divergent package so the fix needs no lookup"
    )


@pytest.mark.parametrize("relative", sorted(EXEMPT_FIXTURES))
def test_exempt_fixtures_are_the_documented_directories(relative: str) -> None:
    """Each exemption matches the manifest beneath it, and nothing else."""
    assert is_exempt(REPO_ROOT / relative / "Cargo.toml", REPO_ROOT), (
        f"the exemption for {relative} must match its own manifest"
    )
    assert not is_exempt(
        REPO_ROOT / "crates" / "rstest-bdd" / "Cargo.toml", REPO_ROOT
    ), "an exemption must not cover an unrelated manifest"


def test_alignment_failures_report_a_fixture_resolving_away_from_the_workspace(
    tmp_path: Path,
) -> None:
    """A fixture pinned to a workspace-absent version is reported.

    The fixture is laid out the way discovery expects (``[workspace]``, a path
    dependency, a sibling lockfile) so this exercises the real check path rather
    than the comparison helper alone.
    """
    fixture = tmp_path / "fixtures" / "drifted"
    fixture.mkdir(parents=True)
    (fixture / "Cargo.toml").write_text(
        '[package]\nname = "drifted-fixture"\nversion = "0.1.0"\n'
        '[dependencies]\nlocal = { path = "../../local" }\n[workspace]\n',
        encoding="utf-8",
    )
    _write_lockfile(
        fixture / "Cargo.lock",
        [("drifted-fixture", "0.1.0", None), ("dashmap", "5.5.3", CRATES_IO)],
    )
    _write_lockfile(
        tmp_path / "Cargo.lock",
        [("dashmap", "6.1.0", CRATES_IO), ("serde", "1.0.228", CRATES_IO)],
    )

    failures = alignment_failures(tmp_path)

    assert len(failures) == 1, "only the drifted fixture must be reported"
    assert "drifted/Cargo.toml" in failures[0], "the report must name the fixture"
    assert "dashmap 5.5.3" in failures[0], "the report must name the drifted package"
    assert "drifted-fixture" not in failures[0], (
        "a fixture's own crate must not be reported as drift"
    )


def test_alignment_passes_when_only_the_own_crate_is_new(tmp_path: Path) -> None:
    """The fixture's own crate is the only legitimate addition to the graph."""
    fixture = tmp_path / "fixtures" / "aligned"
    fixture.mkdir(parents=True)
    (fixture / "Cargo.toml").write_text(
        '[package]\nname = "aligned-fixture"\nversion = "0.1.0"\n'
        '[dependencies]\nlocal = { path = "../../local" }\n[workspace]\n',
        encoding="utf-8",
    )
    _write_lockfile(
        fixture / "Cargo.lock",
        [("aligned-fixture", "0.1.0", None), ("serde", "1.0.228", CRATES_IO)],
    )
    _write_lockfile(tmp_path / "Cargo.lock", [("serde", "1.0.228", CRATES_IO)])

    assert not alignment_failures(tmp_path), (
        "a fixture adding only its own crate must be aligned"
    )


def test_check_alignment_exit_codes(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Aligned fixtures are silent; a failure prints and exits non-zero."""
    probe = tmp_path / "probe"
    fixture = probe / "fixtures" / "drifted"
    fixture.mkdir(parents=True)
    (fixture / "Cargo.toml").write_text(
        '[package]\nname = "drifted-fixture"\nversion = "0.1.0"\n'
        '[dependencies]\nlocal = { path = "../../local" }\n[workspace]\n',
        encoding="utf-8",
    )
    _write_lockfile(
        fixture / "Cargo.lock",
        [("drifted-fixture", "0.1.0", None), ("dashmap", "5.5.3", CRATES_IO)],
    )
    _write_lockfile(probe / "Cargo.lock", [("serde", "1.0.228", CRATES_IO)])

    assert check_alignment(probe) == 1, "drift must fail the gate"
    assert "dashmap 5.5.3" in capsys.readouterr().err, (
        "a drift failure must be printed to standard error"
    )

    _write_lockfile(
        fixture / "Cargo.lock",
        [("drifted-fixture", "0.1.0", None), ("serde", "1.0.228", CRATES_IO)],
    )

    assert check_alignment(probe) == 0, "an aligned fixture must pass the gate"
    assert not capsys.readouterr().err, "an aligned fixture must print nothing"


def test_the_repository_itself_is_aligned(capsys: pytest.CaptureFixture[str]) -> None:
    """Every discovered fixture in this repository reuses the workspace graph.

    This is the regression pin: if a dependency bump re-forks a fixture's
    resolution, this fails with the packages that were duplicated.
    """
    exit_code = check_alignment(REPO_ROOT)
    assert exit_code == 0, capsys.readouterr().err


@pytest.mark.parametrize(
    "fixture",
    [
        "crates/cargo-bdd/tests/fixtures/minimal",
        "crates/rstest-bdd/tests/fixtures/feature_addition",
        "crates/rstest-bdd/tests/fixtures/rebuild_invalidation",
    ],
)
def test_aligned_fixtures_declare_no_unused_dependencies(fixture: str) -> None:
    """Each fixture names only dependencies its sources actually reach.

    An unused requirement still enters the fixture's resolved graph, so it can
    fork a shared crate just as a wrong version can.
    """
    root = REPO_ROOT / fixture
    manifest = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    declared = set(manifest.get("dependencies", {})) | set(
        manifest.get("dev-dependencies", {})
    )
    declared -= {"rstest-bdd", "rstest-bdd-macros"}
    source = "\n".join(
        path.read_text(encoding="utf-8")
        for path in sorted(root.rglob("*.rs"))
        if "target" not in path.parts
    )
    for dependency in sorted(declared):
        crate = dependency.replace("-", "_")
        assert f"{crate}::" in source or f"use {crate}" in source, dependency
