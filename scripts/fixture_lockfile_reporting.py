#!/usr/bin/env python3
"""Failure and success reporting for the fixture-lockfile gate.

Keeping the message formatting and the shared error type apart from the Cargo
plumbing lets the gate script stay under the 400-line budget while the wording
of a stale-lockfile or failed-prefetch failure stays testable in one place.
"""

import dataclasses
import sys
import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    import subprocess
    from pathlib import Path

#: Suggested remediation printed when a lockfile no longer resolves.
REFRESH_HINT = "run 'make update-fixture-lockfiles' to refresh them"


class FixtureLockfileError(RuntimeError):
    """A fixture lockfile is stale, or no fixture manifest was discovered.

    Lives with the reporting helpers rather than the gate script so the
    discovery module and the gate can both raise it without importing each
    other.
    """

    @staticmethod
    def no_manifests_message() -> str:
        """Return the message for an empty discovery result."""
        return (
            "no standalone fixture manifests found; the discovery contract "
            "expects at least one committed fixture lockfile"
        )

    @staticmethod
    def cargo_unavailable_message(cargo: str, manifest: Path, error: OSError) -> str:
        """Return the message for a Cargo executable that could not run."""
        return f"cannot run {cargo} for {manifest}: {error}"


def _failure_message(heading: str) -> cabc.Callable[[Path, list[str], str, str], str]:
    """
    Build the shared Cargo failure-report formatter for *heading*.

    Every gate operation reports a failure the same way and differs only in the
    clause that opens the report, so one factory serves all three messages. The
    returned formatter takes the four parts the gate has in hand: the failing
    manifest, the Cargo command that failed, and the two captured streams.

    Parameters
    ----------
    heading : str
        The opening clause naming the failed operation, carrying whatever
        punctuation that operation's established wording uses.

    Returns
    -------
    cabc.Callable[[Path, list[str], str, str], str]
        A formatter rendering one multi-line failure report.
    """

    def report(
        manifest: Path,
        command: list[str],
        result_stdout: str,
        result_stderr: str,
    ) -> str:
        """Render one failure report under the factory's heading."""
        return (
            f"{heading} {manifest}\n"
            f"command: {' '.join(command)}\n"
            f"cargo output:\n{result_stdout}{result_stderr}"
        )

    return report


# The three gate messages share one report body and differ only by the heading
# that opens it. Each heading keeps the punctuation of the wording it has always
# produced — including the colon-free refresh heading — so no report changes.
stale_failure_message = _failure_message("stale or unusable fixture lockfile:")
refresh_failure_message = _failure_message("refresh failed for")
fetch_failure_message = _failure_message(
    "failed to prefetch fixture dependencies:",
)


def print_failures(failures: list[str]) -> None:
    """
    Print every failure report to standard error.

    Parameters
    ----------
    failures : list[str]
        The rendered failure reports.
    """
    for failure in failures:
        print(failure, file=sys.stderr)


def print_check_summary(total: int, failed: int) -> None:
    """
    Print the validation outcome to standard output or standard error.

    Parameters
    ----------
    total : int
        The number of fixtures the gate checked.
    failed : int
        The number of fixtures whose lockfile is stale.
    """
    if failed:
        print(
            f"{failed} of {total} fixture lockfile(s) are stale; {REFRESH_HINT}",
            file=sys.stderr,
        )
    else:
        print(f"{total} fixture lockfile(s) up to date")


def print_refresh_summary(total: int, failed: int) -> None:
    """
    Print the refresh outcome to standard output.

    Parameters
    ----------
    total : int
        The number of fixtures the refresh regenerated.
    failed : int
        The number of fixtures that still fail after the refresh.
    """
    if failed:
        print(f"{failed} of {total} fixture lockfile(s) still stale", file=sys.stderr)
    else:
        print(f"refreshed {total} fixture lockfile(s)")


def print_fetch_summary(total: int, failed: int) -> None:
    """
    Print the dependency-prefetch outcome to standard output.

    Parameters
    ----------
    total : int
        The number of fixtures the prefetch visited.
    failed : int
        The number of fixtures whose dependencies could not be fetched.
    """
    if failed:
        print(
            f"{failed} of {total} fixture dependency prefetch(es) failed",
            file=sys.stderr,
        )
    else:
        print(f"prefetched dependencies for {total} fixture(s)")


@dataclasses.dataclass(frozen=True, slots=True)
class GateMode:
    """Bundle the callables that distinguish one fixture-gate operation.

    Bundling keeps :func:`check_fixtures`, :func:`refresh_fixtures`, and
    :func:`fetch_fixtures` in the gate script to three call arguments, inside
    the ``max-args`` lint budget.

    Parameters
    ----------
    failure_message : cabc.Callable[[Path, list[str], str, str], str]
        Render one failing manifest and Cargo output as report text.
    print_summary : cabc.Callable[[int, int], None]
        Close the run with the total and failed counts.
    command : cabc.Callable[[Path], list[str]]
        Build the Cargo argv the failure report quotes for reproduction.
    operation : cabc.Callable[[Path], subprocess.CompletedProcess[str]]
        Run that argv for one manifest; a non-zero exit is a failure.
    prepare : cabc.Callable[[Path], object] | None
        Optional per-manifest step before the operation, or None.
    """

    failure_message: cabc.Callable[[Path, list[str], str, str], str]
    print_summary: cabc.Callable[[int, int], None]
    command: cabc.Callable[[Path], list[str]]
    operation: cabc.Callable[[Path], subprocess.CompletedProcess[str]]
    prepare: cabc.Callable[[Path], object] | None = None
