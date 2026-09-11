#!/usr/bin/env python3
"""Failure and success reporting for the fixture-lockfile gate.

Keeping the message formatting and the shared error type apart from the Cargo
plumbing lets the gate script stay under the 400-line budget while the wording
of a stale-lockfile failure stays testable in one place.
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


def stale_failure_message(
    manifest: Path, command: list[str], result_stdout: str, result_stderr: str
) -> str:
    """
    Return the failure text naming the stale manifest with Cargo's output.

    Parameters
    ----------
    manifest : Path
        The fixture whose lockfile failed to resolve.
    command : list[str]
        The Cargo command that failed, for reproduction.
    result_stdout : str
        Captured standard output from the failed Cargo run.
    result_stderr : str
        Captured standard error from the failed Cargo run.

    Returns
    -------
    str
        The multi-line failure report.
    """
    return (
        f"stale or unusable fixture lockfile: {manifest}\n"
        f"command: {' '.join(command)}\n"
        f"cargo output:\n{result_stdout}{result_stderr}"
    )


def refresh_failure_message(
    manifest: Path, command: list[str], result_stdout: str, result_stderr: str
) -> str:
    """
    Return the failure text for a lockfile that is stale after a refresh.

    Parameters
    ----------
    manifest : Path
        The fixture whose lockfile still fails to resolve after refreshing.
    command : list[str]
        The verification command that failed, for reproduction.
    result_stdout : str
        Captured standard output from the failed verification run.
    result_stderr : str
        Captured standard error from the failed verification run.

    Returns
    -------
    str
        The multi-line failure report.
    """
    return (
        f"refresh failed for {manifest}\n"
        f"command: {' '.join(command)}\n"
        f"cargo output:\n{result_stdout}{result_stderr}"
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


@dataclasses.dataclass(frozen=True, slots=True)
class GateMode:
    """Bundle the callables that distinguish one fixture-gate operation.

    Bundling keeps the gate script's entry points to three call arguments,
    inside the ``max-args`` lint budget.

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
