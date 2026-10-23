#!/usr/bin/env python3
"""Failure and success reporting for the fixture-lockfile gate.

Keeping the message formatting and the shared error type apart from the Cargo
plumbing lets the gate script stay under the 400-line budget while the wording
of a stale-lockfile, failed-prefetch, or divergent-fixture failure stays
testable in one place. The alignment report belongs here on the same terms: the
comparison module answers which packages diverged, and the prose that names
them is wording, like every other report this module owns. The prefetch mode's
machine-readable metrics record lives here too, beside the human-readable
summary it accompanies.
"""

import dataclasses
import json
import sys
import time
import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    import subprocess
    from pathlib import Path, PurePath

    # Annotation-only: importing the packages module at runtime would close a
    # cycle, because it imports this module's error type.
    from fixture_lockfile_packages import PackageKey

#: Suggested remediation printed when a lockfile no longer resolves.
REFRESH_HINT = "run 'make update-fixture-lockfiles' to refresh them"

#: Prefix marking the prefetch metrics record, so one grep or line filter finds
#: the record in a job log without parsing the human-readable lines.
PREFETCH_METRICS_PREFIX = "fixture-prefetch-metrics: "

#: Schema version of the prefetch metrics record; bump it when the field set or
#: a field's meaning changes, never when a value changes.
PREFETCH_METRICS_SCHEMA_VERSION = 1

#: Cache outcome recorded when Cargo offers no reliable cache/download
#: evidence. ``cargo fetch`` reports downloads as human-readable progress lines
#: and prints nothing when the cache already has a crate, so the absence of a
#: download line proves nothing and the record never guesses. ``unknown`` is
#: the only bounded value available today; a machine-readable Cargo signal
#: would be needed before a record may say more.
CACHE_OUTCOME_UNKNOWN = "unknown"


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


def _failure_message(
    heading: str,
) -> cabc.Callable[[PurePath, list[str], str, str], str]:
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
    cabc.Callable[[PurePath, list[str], str, str], str]
        A formatter rendering one multi-line failure report.
    """

    def report(
        manifest: PurePath,
        command: list[str],
        result_stdout: str,
        result_stderr: str,
    ) -> str:
        """Render one failure report under the factory's heading."""
        # `as_posix`, not `str`: these reports are read by someone comparing a
        # CI log against a repository path, and on Windows the native
        # separator renders the same fixture with backslashes, so the two
        # platforms name one file two ways. `fixture_lockfile_discovery`
        # already speaks POSIX for the same reason.
        return (
            f"{heading} {manifest.as_posix()}\n"
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


def format_package(key: PackageKey) -> str:
    """Return a package identity as one readable line.

    Parameters
    ----------
    key : PackageKey
        The package to render.

    Returns
    -------
    str
        ``name version`` followed by the source when the package has one.

    Examples
    --------
    >>> format_package(("serde", "1.0.228", None))
    'serde 1.0.228'
    >>> format_package(("serde", "1.0.228", "registry+https://example.invalid"))
    'serde 1.0.228 (registry+https://example.invalid)'
    """
    name, version, source = key
    return f"{name} {version}" if source is None else f"{name} {version} ({source})"


def alignment_failure(manifest: Path, packages: list[PackageKey]) -> str:
    """Render the report for a fixture that resolves outside the workspace.

    The wording lives with the other reports because it is wording: the
    comparison module answers which packages diverged, and the fix a reader
    needs — align the requirements, or seed the lockfile from the root one — is
    a sentence rather than a comparison result.

    Parameters
    ----------
    manifest : Path
        The fixture manifest, named so the report points at the fixture to fix.
    packages : list[PackageKey]
        The divergent packages, already sorted.

    Returns
    -------
    str
        A multi-line report naming every divergent package.
    """
    listed = "\n".join(f"  {format_package(key)}" for key in packages)
    count = len(packages)
    return (
        f"fixture {manifest.as_posix()} resolves {count} package(s) the "
        f"workspace does not:\n{listed}\n"
        f"each one is a compiled unit the nested build cannot reuse from the "
        f"workspace build; align the fixture's requirements with the workspace "
        f"(or seed its lockfile from the root Cargo.lock) and run "
        f"'make update-fixture-lockfiles'"
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


def prefetch_metrics_record(
    total: int,
    succeeded: int,
    failed: int,
    elapsed_ms: int,
) -> str:
    """Render the bounded prefetch metrics record for one ``--fetch`` run.

    The record carries counts, a duration, the outcome, and the cache outcome —
    never a manifest path, crate name, command line, environment value, URL, or
    Cargo output — so an aggregating reader can parse it without learning
    anything about the machine that produced it. Field order is fixed and the
    JSON is emitted compactly, so the same numbers always render the same line.

    Parameters
    ----------
    total : int
        The number of fixtures the prefetch visited.
    succeeded : int
        The number of fixtures whose dependencies are now cached.
    failed : int
        The number of fixtures whose ``cargo fetch`` failed.
    elapsed_ms : int
        Wall-clock milliseconds the whole prefetch took.

    Returns
    -------
    str
        The prefixed JSON line standing for one prefetch run.

    Examples
    --------
    >>> expected = (
    ...     'fixture-prefetch-metrics: {"schema_version":1,"total":2,'
    ...     '"succeeded":2,"failed":0,"elapsed_ms":7,"outcome":"success",'
    ...     '"cache_outcome":"unknown"}'
    ... )
    >>> prefetch_metrics_record(2, 2, 0, 7) == expected
    True
    """
    return PREFETCH_METRICS_PREFIX + json.dumps(
        {
            "schema_version": PREFETCH_METRICS_SCHEMA_VERSION,
            "total": total,
            "succeeded": succeeded,
            "failed": failed,
            "elapsed_ms": elapsed_ms,
            "outcome": "failure" if failed else "success",
            "cache_outcome": CACHE_OUTCOME_UNKNOWN,
        },
        separators=(",", ":"),
    )


def print_prefetch_metrics(total: int, failed: int, elapsed_ms: int) -> None:
    """
    Print the prefetch metrics record beside the summary for the same outcome.

    A clean run carries its record on standard output with the success summary;
    a failed run carries it on standard error with the failure reports, so a
    reader capturing only one stream never reads a failed prefetch as a clean
    one.

    Parameters
    ----------
    total : int
        The number of fixtures the prefetch visited.
    failed : int
        The number of fixtures whose dependencies could not be fetched.
    elapsed_ms : int
        Wall-clock milliseconds the whole prefetch took.
    """
    record = prefetch_metrics_record(total, total - failed, failed, elapsed_ms)
    print(record, file=sys.stderr if failed else sys.stdout)


@dataclasses.dataclass(frozen=True, slots=True)
class GateMode:
    """Bundle the callables that distinguish one fixture-gate operation.

    Bundling keeps :func:`check_fixtures`, :func:`refresh_fixtures`, and
    :func:`fetch_fixtures` in the gate script to three call arguments, inside
    the ``max-args`` lint budget.

    Parameters
    ----------
    failure_message : cabc.Callable[[PurePath, list[str], str, str], str]
        Render one failing manifest and Cargo output as report text.
    print_summary : cabc.Callable[[int, int], None]
        Close the run with the total and failed counts.
    command : cabc.Callable[[Path], list[str]]
        Build the Cargo argv the failure report quotes for reproduction.
    operation : cabc.Callable[[Path], subprocess.CompletedProcess[str]]
        Run that argv for one manifest; a non-zero exit is a failure.
    prepare : cabc.Callable[
            [Path, Path], subprocess.CompletedProcess[str] | None
        ]
        | None
        Optional per-manifest step before the operation, taking the repository
        root and the manifest. A non-zero result fails that fixture without
        validating it: a failed preparation restores the committed lockfile,
        and that restored lockfile would validate cleanly. None for a mode that
        prepares nothing.
    report_metrics : cabc.Callable[[int, int, int], None] | None
        Optional machine-readable record of the whole run, taking the fixture
        total, the failed count, and the elapsed milliseconds; None for a mode
        that emits no record.
    clock : cabc.Callable[[], float]
        The monotonic clock that record is timed with. It is read only by a
        mode that sets *report_metrics*, so the validating modes never read a
        clock at all.
    """

    failure_message: cabc.Callable[[PurePath, list[str], str, str], str]
    print_summary: cabc.Callable[[int, int], None]
    command: cabc.Callable[[Path], list[str]]
    operation: cabc.Callable[[Path], subprocess.CompletedProcess[str]]
    prepare: (
        cabc.Callable[[Path, Path], subprocess.CompletedProcess[str] | None] | None
    ) = None
    report_metrics: cabc.Callable[[int, int, int], None] | None = None
    clock: cabc.Callable[[], float] = time.monotonic
