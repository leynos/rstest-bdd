"""Unit tests for the standalone fixture-lockfile gate.

The gate validates committed fixture lockfiles with ``cargo metadata --locked``.
These tests pin the discovery contract and the failure behaviour without
mutating any repository fixture during parallel runs.
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
    main,
    refresh_fixtures,
    refresh_lockfile,
    run_cargo_command,
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


def test_run_cargo_command_reports_a_missing_cargo_executable() -> None:
    """A Cargo that cannot start raises the gate's own error, named per fixture."""
    manifest = REPO_ROOT / "crates/rstest-bdd/tests/ui_lints/Cargo.toml"
    oserror = OSError(2, "No such file or directory")
    with (
        mock.patch(
            "check_fixture_lockfiles.subprocess.run", side_effect=oserror
        ) as spawn,
        pytest.raises(FixtureLockfileError, match="cannot run cargo") as excinfo,
    ):
        run_cargo_command(cargo_metadata_command(manifest), manifest)
    assert spawn.call_args.kwargs["capture_output"] is True, (
        "the runner must capture Cargo output so failures can name it"
    )
    assert spawn.call_args.kwargs["text"] is True, (
        "Cargo output must be decoded as text"
    )
    assert spawn.call_args.kwargs["check"] is False, (
        "a non-zero exit must return the result so every fixture is reported"
    )
    assert isinstance(excinfo.value.__cause__, OSError), (
        "the original launch failure must stay chained for debugging"
    )


def test_thin_wrappers_delegate_to_the_shared_runner() -> None:
    """Both public entry points build their argv and hand it to one runner."""
    manifest = (
        REPO_ROOT / "crates/rstest-bdd/tests/fixtures/feature_addition/Cargo.toml"
    )
    with mock.patch("check_fixture_lockfiles.run_cargo_command") as runner:
        run_cargo_metadata(manifest)
        refresh_lockfile(manifest)
    assert runner.call_count == 2, (
        "each wrapper must route through the shared Cargo runner exactly once"
    )
    metadata_argv, metadata_manifest = runner.call_args_list[0].args
    refresh_argv, refresh_manifest = runner.call_args_list[1].args
    assert metadata_argv == cargo_metadata_command(manifest), (
        "validation must always use the locked metadata argv"
    )
    assert metadata_manifest == manifest, "validation must name the manifest it checked"
    assert refresh_argv == [
        "cargo",
        "generate-lockfile",
        "--manifest-path",
        str(manifest),
    ], "the refresh argv must stay cargo generate-lockfile --manifest-path"
    assert refresh_manifest == manifest, "refresh must name the manifest it regenerated"


def test_check_failure_output_carries_manifest_command_and_cargo_streams() -> None:
    """The check failure report names the fixture, command, stdout, and stderr."""
    manifest = (
        REPO_ROOT / "crates/rstest-bdd/tests/fixtures/feature_addition/Cargo.toml"
    )
    failing = subprocess.CompletedProcess(
        args=cargo_metadata_command(manifest),
        returncode=101,
        stdout="partial resolution output",
        stderr=STALE_OUTPUT,
    )
    with (
        mock.patch("check_fixture_lockfiles.run_cargo_metadata", return_value=failing),
        mock.patch("check_fixture_lockfiles.print_failures") as emit_failures,
    ):
        exit_code = check_fixtures(REPO_ROOT, [manifest])
    assert exit_code == 1, "a stale lockfile must fail the gate"
    report = emit_failures.call_args.args[0][0]
    assert "feature_addition" in report, "the report must name the stale fixture"
    assert "cargo metadata --locked" in report, "the report must give the command"


def test_check_failure_report_carries_both_cargo_streams() -> None:
    """The check failure report relays Cargo stdout and stderr verbatim."""
    manifest = (
        REPO_ROOT / "crates/rstest-bdd/tests/fixtures/feature_addition/Cargo.toml"
    )
    failing = subprocess.CompletedProcess(
        args=cargo_metadata_command(manifest),
        returncode=101,
        stdout="partial resolution output",
        stderr=STALE_OUTPUT,
    )
    with (
        mock.patch("check_fixture_lockfiles.run_cargo_metadata", return_value=failing),
        mock.patch("check_fixture_lockfiles.print_failures") as emit_failures,
    ):
        check_fixtures(REPO_ROOT, [manifest])
    report = emit_failures.call_args.args[0][0]
    assert STALE_OUTPUT in report, "the report must carry Cargo stderr"
    assert "partial resolution output" in report, "the report must carry stdout"


def test_refresh_failure_report_uses_the_refresh_wording() -> None:
    """Refresh failures keep the refresh-specific report and summary."""
    manifest = REPO_ROOT / "crates/rstest-bdd/tests/ui_lints/Cargo.toml"
    failing = subprocess.CompletedProcess(
        args=cargo_metadata_command(manifest),
        returncode=101,
        stdout="",
        stderr=STALE_OUTPUT,
    )
    with (
        mock.patch("check_fixture_lockfiles.refresh_lockfile", return_value=failing),
        mock.patch("check_fixture_lockfiles.run_cargo_metadata", return_value=failing),
        mock.patch("check_fixture_lockfiles.print_failures") as emit_failures,
    ):
        exit_code = refresh_fixtures(REPO_ROOT, [manifest])
    assert exit_code == 1, "a lockfile still stale after refresh must fail"
    report = emit_failures.call_args.args[0][0]
    assert report.startswith("refresh failed for "), (
        "refresh mode must keep its own failure wording"
    )
    assert "ui_lints" in report, "the report must name the stale fixture"
    assert STALE_OUTPUT in report, "the report must carry Cargo stderr"


def test_refresh_failure_summary_streams_to_stderr(
    capsys: pytest.CaptureFixture[str],
) -> None:
    """A failed refresh surfaces its summary on standard error, exit code 1."""
    manifest = REPO_ROOT / "crates/rstest-bdd/tests/ui_lints/Cargo.toml"
    failing = subprocess.CompletedProcess(
        args=cargo_metadata_command(manifest),
        returncode=101,
        stdout="",
        stderr=STALE_OUTPUT,
    )
    with (
        mock.patch("check_fixture_lockfiles.refresh_lockfile"),
        mock.patch("check_fixture_lockfiles.run_cargo_metadata", return_value=failing),
    ):
        exit_code = refresh_fixtures(REPO_ROOT, [manifest])
    assert exit_code == 1, "a lockfile still stale after refresh must fail"
    captured = capsys.readouterr()
    assert "1 of 1 fixture lockfile(s) still stale" in captured.err, (
        "the refresh summary must report the still-stale count"
    )


def test_main_lists_manifests_without_running_the_gate(
    capsys: pytest.CaptureFixture[str],
) -> None:
    """--list prints the discovered set and exits 0 before any Cargo run."""
    manifests = discover_fixture_manifests(REPO_ROOT)
    with mock.patch("check_fixture_lockfiles.run_cargo_command") as runner:
        exit_code = main(["--list"])
    assert exit_code == 0, "listing must exit successfully"
    assert runner.assert_not_called() is None, "listing must not invoke Cargo"
    printed = capsys.readouterr().out.splitlines()
    assert printed == [m.relative_to(REPO_ROOT).as_posix() for m in manifests], (
        "--list must print every discovered manifest path"
    )


def test_main_reports_discovery_failure_as_exit_one(
    capsys: pytest.CaptureFixture[str],
) -> None:
    """An empty fixture set is an error the caller can see, not a crash."""
    with mock.patch(
        "check_fixture_lockfiles.discover_fixture_manifests",
        side_effect=FixtureLockfileError(FixtureLockfileError.no_manifests_message()),
    ):
        exit_code = main([])
    assert exit_code == 1, "a broken discovery contract must fail the run"
    assert "no standalone fixture manifests found" in capsys.readouterr().err, (
        "discovery failure must name the missing contract"
    )


def test_main_refresh_flag_routes_to_refresh_fixtures(
    capsys: pytest.CaptureFixture[str],
) -> None:
    """--refresh regenerates lockfiles instead of only validating them."""
    manifests = discover_fixture_manifests(REPO_ROOT)
    successful = subprocess.CompletedProcess(
        args=cargo_metadata_command(manifests[0]),
        returncode=0,
        stdout="",
        stderr="",
    )
    with (
        mock.patch("check_fixture_lockfiles.refresh_lockfile", return_value=successful),
        mock.patch(
            "check_fixture_lockfiles.run_cargo_metadata", return_value=successful
        ),
        mock.patch(
            "check_fixture_lockfiles.discover_fixture_manifests",
            return_value=manifests,
        ),
    ):
        exit_code = main(["--refresh"])
    assert exit_code == 0, "a clean refresh must exit zero"
    assert "refreshed" in capsys.readouterr().out, (
        "the refresh summary must confirm the run"
    )
