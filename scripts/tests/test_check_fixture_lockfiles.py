"""Unit tests for the standalone fixture-lockfile gate.

The gate discovers every workspace opt-out manifest that carries local ``path``
dependencies and a committed lockfile, then validates each with
``cargo metadata --locked``. These tests pin the discovery contract (so a new
fixture is picked up automatically) and the failure behaviour (a stale lockfile
must fail before any behavioural nested-Cargo test can run), without mutating
any repository fixture during parallel runs.
"""

import subprocess  # ruff: ignore[suspicious-subprocess-import] - tests build stand-in CompletedProcess values without running anything.
from pathlib import Path
from unittest import mock

import check_fixture_lockfiles
import pytest
from check_fixture_lockfiles import (
    FixtureLockfileError,
    cargo_metadata_command,
    check_fixtures,
    discover_fixture_manifests,
    is_staged_fixture,
    is_workspace_root,
    refresh_fixtures,
    run_cargo_metadata,
)

REPO_ROOT = Path(__file__).resolve().parents[2]

STALE_OUTPUT = (
    "error: cannot update the lock file ... because --locked was passed to prevent this"
)


def test_discovery_includes_feature_addition_fixture() -> None:
    """The authoritative fixture set covers the scenario-addition fixture."""
    manifests = discover_fixture_manifests(REPO_ROOT)
    names = {manifest.parent.name for manifest in manifests}
    assert "feature_addition" in names, (
        "discovery must include the feature_addition fixture; a fixture added "
        "to the set without a committed lockfile would otherwise escape the gate"
    )


def test_discovery_includes_every_committed_standalone_lockfile() -> None:
    """Every standalone lockfile on disk is covered by discovery."""
    manifests = discover_fixture_manifests(REPO_ROOT)
    lockfiles = {manifest.parent / "Cargo.lock" for manifest in manifests}
    for lockfile in (
        REPO_ROOT / "crates/cargo-bdd/tests/fixtures/minimal/Cargo.lock",
        REPO_ROOT / "crates/rstest-bdd/tests/fixtures/feature_addition/Cargo.lock",
        REPO_ROOT / "crates/rstest-bdd/tests/fixtures/rebuild_invalidation/Cargo.lock",
        REPO_ROOT / "crates/rstest-bdd/tests/ui_lints/Cargo.lock",
        REPO_ROOT / "tests/fixtures/published-gpui-0-2-2/Cargo.lock",
    ):
        assert lockfile in lockfiles, (
            f"{lockfile.relative_to(REPO_ROOT)} must stay inside the "
            "authoritative standalone fixture set"
        )


def test_discovery_excludes_the_workspace_root_and_staged_fixture() -> None:
    """The root workspace and the staged e2e fixture are out of scope."""
    manifests = discover_fixture_manifests(REPO_ROOT)
    relative = {manifest.relative_to(REPO_ROOT).as_posix() for manifest in manifests}
    assert "Cargo.toml" not in relative, (
        "the workspace root resolves through the workspace lockfile and is not "
        "a standalone fixture"
    )
    assert "tests/fixtures/published-gpui-e2e/Cargo.toml" not in relative, (
        "the staged e2e fixture resolves against target/ artefacts, not a lockfile"
    )


def test_every_discovered_manifest_matches_its_lockfile() -> None:
    """Each committed fixture lockfile resolves against its manifest."""
    for manifest in discover_fixture_manifests(REPO_ROOT):
        result = run_cargo_metadata(manifest)
        assert result.returncode == 0, (
            f"stale fixture lockfile for {manifest.relative_to(REPO_ROOT)}:\n"
            f"{result.stdout}{result.stderr}"
        )


def test_stale_lockfile_fails_with_manifest_path_and_cargo_output() -> None:
    """A failing cargo metadata surfaces the manifest path and Cargo output."""
    manifest = (
        REPO_ROOT / "crates/rstest-bdd/tests/fixtures/feature_addition/Cargo.toml"
    )
    failing = subprocess.CompletedProcess(
        args=cargo_metadata_command(manifest),
        returncode=101,
        stdout="",
        stderr=STALE_OUTPUT,
    )
    with mock.patch("check_fixture_lockfiles.run_cargo_metadata", return_value=failing):
        exit_code = check_fixtures(REPO_ROOT, [manifest])
    assert exit_code == 1, "a stale lockfile must fail the gate"


def test_check_gate_fails_before_nested_cargo_tests_run() -> None:
    """A stale lockfile fails the gate before behavioural tests need Cargo.

    The nested rebuild-invalidation experiments only pass when every fixture
    lockfile resolves. A failing ``cargo metadata`` therefore reds the gate
    itself, and the behavioural suites never reach a nested-Cargo assertion
    that could mask the drift. The gate's failure path is asserted without
    mutating any repository fixture: the stubbed Cargo output stands in for a
    stale lockfile, which keeps the test safe under parallel execution.
    """
    manifests = discover_fixture_manifests(REPO_ROOT)
    failing = subprocess.CompletedProcess(
        args=cargo_metadata_command(manifests[0]),
        returncode=101,
        stdout="",
        stderr=STALE_OUTPUT,
    )
    with (
        mock.patch(
            "check_fixture_lockfiles.run_cargo_metadata", return_value=failing
        ) as metadata,
        mock.patch("check_fixture_lockfiles.discover_fixture_manifests") as discover,
    ):
        discover.return_value = manifests
        exit_code = check_fixtures(REPO_ROOT, manifests)
    assert exit_code == 1, "a stale fixture lockfile must fail the gate"
    assert metadata.call_count == len(manifests), (
        "the gate must check every discovered fixture, not stop at the first"
    )


def test_refresh_mode_regenerates_before_validating() -> None:
    """Refresh mode regenerates each lockfile before locked-metadata validation."""
    manifests = discover_fixture_manifests(REPO_ROOT)
    events: list[str] = []
    observed: list[Path] = []
    successful = subprocess.CompletedProcess(
        args=cargo_metadata_command(manifests[0]),
        returncode=0,
        stdout="",
        stderr="",
    )

    def record_refresh(manifest: Path) -> None:
        """Record a refresh event in the shared call-order log."""
        events.append("refresh")
        observed.append(manifest)

    def record_metadata(manifest: Path) -> subprocess.CompletedProcess[str]:
        """Record a validation event in the shared call-order log."""
        events.append("metadata")
        observed.append(manifest)
        return successful

    with (
        mock.patch.object(
            check_fixture_lockfiles, "refresh_lockfile", side_effect=record_refresh
        ),
        mock.patch.object(
            check_fixture_lockfiles, "run_cargo_metadata", side_effect=record_metadata
        ),
    ):
        exit_code = refresh_fixtures(REPO_ROOT, manifests)
    assert exit_code == 0, "valid refreshed fixtures must pass the gate"
    assert events == ["refresh", "metadata"] * len(manifests), (
        "each fixture must be regenerated before its locked-metadata validation"
    )
    assert observed == [m for m in manifests for _ in range(2)], (
        "every discovered fixture must be visited in order"
    )


def test_workspace_root_is_never_a_fixture() -> None:
    """The workspace root manifest is excluded from the fixture gate."""
    assert is_workspace_root(REPO_ROOT / "Cargo.toml", REPO_ROOT), (
        "the workspace root manifest must never be treated as a fixture"
    )
    assert not is_workspace_root(
        REPO_ROOT / "crates/rstest-bdd/tests/ui_lints/Cargo.toml", REPO_ROOT
    ), "a standalone fixture manifest must stay inside the gate"


def test_staged_fixture_is_excluded_from_discovery() -> None:
    """The target/-staged e2e fixture is validated by its own targets."""
    staged = REPO_ROOT / "tests/fixtures/published-gpui-e2e/Cargo.toml"
    assert is_staged_fixture(staged, REPO_ROOT), (
        "the staged e2e fixture must be excluded from discovery"
    )
    assert not is_staged_fixture(
        REPO_ROOT / "tests/fixtures/published-gpui-0-2-2/Cargo.toml", REPO_ROOT
    ), "the published 0.2.2 fixture stays inside the gate"


def test_discovery_error_names_the_missing_contract() -> None:
    """An empty fixture set fails with a discoverable reason."""
    with (
        mock.patch("check_fixture_lockfiles.iter_cargo_manifests", return_value=[]),
        pytest.raises(FixtureLockfileError, match="no standalone fixture"),
    ):
        discover_fixture_manifests(REPO_ROOT)
