"""Unit tests for the fixture-lockfile reporting helpers."""

import typing as typ
from pathlib import Path

import fixture_lockfile_reporting
import pytest
from fixture_lockfile_reporting import (
    fetch_failure_message,
    print_failures,
    refresh_failure_message,
    stale_failure_message,
)

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: A manifest path the tests never touch: they pin pure report formatting.
MANIFEST = Path("/repo/crates/rstest-bdd/tests/ui_lints/Cargo.toml")

#: The validation command and the prefetch command the gate passes in.
METADATA_COMMAND = ["cargo", "metadata", "--locked", "--manifest-path", str(MANIFEST)]
FETCH_COMMAND = ["cargo", "fetch", "--locked", "--manifest-path", str(MANIFEST)]

#: Each message's opening clause, carrying its established punctuation. The
#: refresh heading has never ended in a colon, unlike its two siblings.
STALE_HEADING = "stale or unusable fixture lockfile:"
REFRESH_HEADING = "refresh failed for"
FETCH_HEADING = "failed to prefetch fixture dependencies:"

STDOUT = "resolving dependencies\n"
STDERR = "error: failed to update\n"


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


@pytest.mark.parametrize(
    ("formatter", "command", "heading"),
    [
        (stale_failure_message, METADATA_COMMAND, STALE_HEADING),
        (refresh_failure_message, METADATA_COMMAND, REFRESH_HEADING),
        (fetch_failure_message, FETCH_COMMAND, FETCH_HEADING),
    ],
    ids=["stale", "refresh", "fetch"],
)
def test_failure_message_reports_the_manifest_the_command_and_the_output(
    formatter: cabc.Callable[[Path, list[str], str, str], str],
    command: list[str],
    heading: str,
) -> None:
    """Every report spells out the heading, the command, and both streams."""
    assert formatter(MANIFEST, command, STDOUT, STDERR) == (
        f"{heading} {MANIFEST}\n"
        f"command: {' '.join(command)}\n"
        "cargo output:\n"
        f"{STDOUT}"
        f"{STDERR}"
    ), f"the {heading!r} report must keep the gate's established wording"
