"""Unit tests for the fixture-lockfile reporting helpers."""

import typing as typ

import fixture_lockfile_reporting
from fixture_lockfile_reporting import print_failures

if typ.TYPE_CHECKING:
    import pytest


def test_print_failures_writes_each_report_to_stderr(
    capsys: pytest.CaptureFixture[str],
) -> None:
    """Every failure report reaches standard error in order."""
    print_failures(["first failure", "second failure"])
    captured = capsys.readouterr()
    assert captured.err == "first failure\nsecond failure\n", (
        "every failure report must reach standard error in order"
    )


def test_check_summary_streams_hint_on_stale_and_ok_on_clean(
    capsys: pytest.CaptureFixture[str],
) -> None:
    """The stale summary carries the remediation hint; the clean one does not."""
    fixture_lockfile_reporting.print_check_summary(5, 2)
    stale = capsys.readouterr()
    assert "2 of 5" in stale.err, "the stale summary must count both totals"
    assert "make update-fixture-lockfiles" in stale.err, (
        "stale output must tell the user how to fix"
    )

    fixture_lockfile_reporting.print_check_summary(5, 0)
    clean = capsys.readouterr()
    assert not clean.err, "a clean summary must not use standard error"
    assert clean.out == "5 fixture lockfile(s) up to date\n", (
        "the clean summary must confirm the count on standard output"
    )


def test_refresh_summary_reports_both_outcomes(
    capsys: pytest.CaptureFixture[str],
) -> None:
    """The refresh summary prints the still-stale count or the success line."""
    fixture_lockfile_reporting.print_refresh_summary(5, 2)
    failed = capsys.readouterr()
    assert failed.err == "2 of 5 fixture lockfile(s) still stale\n", (
        "the refresh summary must report the still-stale count"
    )

    fixture_lockfile_reporting.print_refresh_summary(5, 0)
    refreshed = capsys.readouterr()
    assert not refreshed.err, "a successful refresh must not use standard error"
    assert refreshed.out == "refreshed 5 fixture lockfile(s)\n", (
        "the success line must confirm the refreshed count"
    )
