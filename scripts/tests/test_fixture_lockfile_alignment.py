"""Unit tests for the fixture alignment gate.

The gate's value is entirely in how it fails: a check that cannot report drift,
or that reports a fixture's own crate as drift, would either pass over the
regression it exists to catch or cry wolf on every run. These tests pin the
comparison, the exemption rules, and the reports.
"""

import re
import tomllib
import typing as typ
from pathlib import Path

import pytest
from fixture_lockfile_alignment import (
    EXEMPT_FIXTURES,
    alignment_divergences,
    alignment_failure,
    alignment_failures,
    check_alignment,
    divergent_packages,
    format_package,
    is_exempt,
)

if typ.TYPE_CHECKING:
    import collections.abc as cabc

REPO_ROOT = Path(__file__).resolve().parents[2]

CRATES_IO = "registry+https://github.com/rust-lang/crates.io-index"

#: Dependencies a fixture's own sources never name because a macro expansion
#: emits them. `#[scenario]`, `#[given]`, `scenarios!` and their siblings all
#: expand to `#[rstest::rstest]`, so the requirement is real even though no
#: `use rstest` or `rstest::` path appears in the fixture's text.
MACRO_GENERATED_DEPENDENCIES = frozenset({"rstest"})


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
    tmp_path: Path, write_manifest: cabc.Callable[[Path, str], Path]
) -> None:
    """A fixture pinned to a workspace-absent version is reported."""
    manifest = write_manifest(tmp_path, "drifted")
    _write_lockfile(
        manifest.parent / "Cargo.lock",
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


def test_alignment_passes_when_only_the_own_crate_is_new(
    tmp_path: Path, write_manifest: cabc.Callable[[Path, str], Path]
) -> None:
    """The fixture's own crate is the only legitimate addition to the graph."""
    manifest = write_manifest(tmp_path, "aligned")
    _write_lockfile(
        manifest.parent / "Cargo.lock",
        [("aligned-fixture", "0.1.0", None), ("serde", "1.0.228", CRATES_IO)],
    )
    _write_lockfile(tmp_path / "Cargo.lock", [("serde", "1.0.228", CRATES_IO)])

    assert not alignment_failures(tmp_path), (
        "a fixture adding only its own crate must be aligned"
    )


def test_alignment_divergences_are_the_query_behind_the_reports(
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
    write_manifest: cabc.Callable[[Path, str], Path],
) -> None:
    """The comparison answers with structure; only the gate prints.

    A caller that needs the divergent packages rather than prose reads the
    query, so it must be safe to call for its answer alone: nothing it does may
    write to a stream. The rendering and the stream output belong to the
    report and the gate, which is what the assertions here separate.
    """
    manifest = write_manifest(tmp_path, "drifted")
    _write_lockfile(
        manifest.parent / "Cargo.lock",
        [("drifted-fixture", "0.1.0", None), ("dashmap", "5.5.3", CRATES_IO)],
    )
    _write_lockfile(tmp_path / "Cargo.lock", [("serde", "1.0.228", CRATES_IO)])

    divergences = alignment_divergences(tmp_path)

    assert divergences == [(manifest, [("dashmap", "5.5.3", CRATES_IO)])], (
        "the query must pair each diverging manifest with its own packages only"
    )
    captured = capsys.readouterr()
    assert not captured.out, (
        "the query must write nothing to standard output; rendering belongs to "
        "the report and the gate"
    )
    assert not captured.err, (
        "the query must write nothing to standard error; stream output belongs "
        "to the report and the gate"
    )

    assert check_alignment(tmp_path) == 1, "the gate must report the drift"
    assert "dashmap 5.5.3" in capsys.readouterr().err, (
        "the gate owns the stream output the query deliberately omits"
    )


def test_check_alignment_exit_codes(
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
    write_manifest: cabc.Callable[[Path, str], Path],
) -> None:
    """Aligned fixtures are silent; a failure prints and exits non-zero."""
    probe = tmp_path / "probe"
    manifest = write_manifest(probe, "drifted")
    _write_lockfile(
        manifest.parent / "Cargo.lock",
        [("drifted-fixture", "0.1.0", None), ("dashmap", "5.5.3", CRATES_IO)],
    )
    _write_lockfile(probe / "Cargo.lock", [("serde", "1.0.228", CRATES_IO)])

    assert check_alignment(probe) == 1, "drift must fail the gate"
    assert "dashmap 5.5.3" in capsys.readouterr().err, (
        "a drift failure must be printed to standard error"
    )

    _write_lockfile(
        manifest.parent / "Cargo.lock",
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


def _declares_crate(source: str, crate: str) -> bool:
    """Return whether *source* reaches *crate* by its whole identifier.

    Whole-identifier matching is the point: a substring test accepts `serde`
    when the source says `use serde_json`, so an unused requirement of a crate
    whose name prefixes a used one would pass the check that exists to catch
    exactly that.

    Returns
    -------
    bool
        ``True`` when *source* names *crate* as a whole identifier.
    """
    pattern = rf"\b{re.escape(crate)}::|\buse\s+{re.escape(crate)}\b"
    return re.search(pattern, source) is not None


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
    declared -= MACRO_GENERATED_DEPENDENCIES
    source = "\n".join(
        path.read_text(encoding="utf-8")
        for path in sorted(root.rglob("*.rs"))
        if "target" not in path.parts
    )
    for dependency in sorted(declared):
        crate = dependency.replace("-", "_")
        assert _declares_crate(source, crate), (
            f"{fixture} declares `{dependency}` but no source reaches it"
        )


def test_dependency_check_rejects_a_prefix_only_match() -> None:
    """A `serde` requirement is not satisfied by `use serde_json` alone.

    The regression this pins: the check exists to find requirements nothing
    imports, and a prefix match would report an unused `serde` as used.
    """
    assert _declares_crate("use serde_json::Value;", "serde") is False, (
        "a longer crate name must not satisfy a shorter requirement"
    )
    assert _declares_crate("use serde::Serialize;", "serde") is True, (
        "a whole-identifier import must satisfy the requirement"
    )
    assert _declares_crate("let _ = serde::json::to_string;", "serde") is True, (
        "a whole-identifier path must satisfy the requirement"
    )
