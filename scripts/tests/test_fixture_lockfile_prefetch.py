"""Unit tests for the standalone fixture dependency prefetch.

The mutation lane runs this mode before cargo-mutants so its nested fixture
builds resolve from a warm registry cache while ``--offline``; these tests pin
that the mode fetches every discovered fixture and reports failures without
validating anything itself.
"""

import subprocess  # ruff: ignore[suspicious-subprocess-import] - tests build stand-in CompletedProcess values without running anything.
from pathlib import Path
from unittest import mock

import pytest
from check_fixture_lockfiles import (
    cargo_fetch_command,
    fetch_fixture_dependencies,
    fetch_fixtures,
    main,
)
from fixture_lockfile_discovery import discover_fixture_manifests

REPO_ROOT = Path(__file__).resolve().parents[2]

FETCH_FAILURE = "error: failed to download `proc-macro-error-attr3 v3.1.0`"


def test_prefetch_fetches_every_discovered_fixture() -> None:
    """Every discovered fixture is warmed with locked ``cargo fetch``."""
    manifests = discover_fixture_manifests(REPO_ROOT)
    successful = subprocess.CompletedProcess(
        args=cargo_fetch_command(manifests[0]), returncode=0, stdout="", stderr=""
    )
    with mock.patch(
        "check_fixture_lockfiles.run_cargo_command", return_value=successful
    ) as runner:
        exit_code = fetch_fixtures(REPO_ROOT, manifests)
    assert exit_code == 0, "a clean prefetch must pass"
    assert [call.args for call in runner.call_args_list] == [
        (cargo_fetch_command(manifest), manifest) for manifest in manifests
    ], "the prefetch must warm every discovered fixture with its locked fetch argv"


def test_prefetch_failure_report_names_the_fixture_and_cargo_output() -> None:
    """A failed fetch names the fixture, the command, and Cargo's output."""
    manifest = REPO_ROOT / "crates/rstest-bdd/tests/fixtures/rebuild_invalidation/Cargo.toml"
    failing = subprocess.CompletedProcess(
        args=cargo_fetch_command(manifest),
        returncode=101,
        stdout="",
        stderr=FETCH_FAILURE,
    )
    with (
        mock.patch(
            "check_fixture_lockfiles.fetch_fixture_dependencies", return_value=failing
        ),
        mock.patch("check_fixture_lockfiles.print_failures") as emit_failures,
    ):
        exit_code = fetch_fixtures(REPO_ROOT, [manifest])
    assert exit_code == 1, "a failed prefetch must fail the run"
    report = emit_failures.call_args.args[0][0]
    assert report.startswith("failed to prefetch fixture dependencies: "), (
        "the prefetch must keep its own failure wording"
    )
    assert "crates/rstest-bdd/tests/fixtures/rebuild_invalidation/Cargo.toml" in report, (
        "the report must name the fixture whose dependencies did not download"
    )
    assert f"command: {' '.join(cargo_fetch_command(manifest))}" in report, (
        "the report must quote the command for reproduction"
    )
    assert FETCH_FAILURE in report, "the report must carry Cargo stderr"


def test_prefetch_failure_summary_streams_to_stderr(
    capsys: pytest.CaptureFixture[str],
) -> None:
    """A failed prefetch surfaces its summary on standard error, exit code 1."""
    manifest = REPO_ROOT / "crates/rstest-bdd/tests/fixtures/rebuild_invalidation/Cargo.toml"
    failing = subprocess.CompletedProcess(
        args=cargo_fetch_command(manifest),
        returncode=101,
        stdout="",
        stderr=FETCH_FAILURE,
    )
    with mock.patch(
        "check_fixture_lockfiles.fetch_fixture_dependencies", return_value=failing
    ):
        exit_code = fetch_fixtures(REPO_ROOT, [manifest])
    assert exit_code == 1, "a failed prefetch must fail the run"
    captured = capsys.readouterr()
    assert "1 of 1 fixture dependency prefetch(es) failed" in captured.err, (
        "the prefetch summary must report the failed count on standard error"
    )


def test_main_fetch_flag_routes_to_prefetch_without_validating(
    capsys: pytest.CaptureFixture[str],
) -> None:
    """--fetch warms the cache instead of validating the lockfiles."""
    manifests = discover_fixture_manifests(REPO_ROOT)
    successful = subprocess.CompletedProcess(
        args=cargo_fetch_command(manifests[0]), returncode=0, stdout="", stderr=""
    )
    with (
        mock.patch(
            "check_fixture_lockfiles.discover_fixture_manifests", return_value=manifests
        ),
        mock.patch(
            "check_fixture_lockfiles.fetch_fixture_dependencies", return_value=successful
        ),
        mock.patch("check_fixture_lockfiles.run_cargo_metadata") as metadata,
    ):
        exit_code = main(["--fetch"])
    assert exit_code == 0, "a clean prefetch must exit zero"
    assert metadata.call_count == 0, (
        "the prefetch mode must not stand in for lockfile validation"
    )
    assert f"prefetched dependencies for {len(manifests)} fixture(s)" in (
        capsys.readouterr().out
    ), "the prefetch summary must confirm the run"
