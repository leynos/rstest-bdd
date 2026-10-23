#!/usr/bin/env python3
"""Validate, refresh, and prefetch the standalone fixture lockfiles.

Standalone fixture crates opt out of the workspace with a ``[workspace]``
stanza, use local ``path =`` dependencies, and commit their own ``Cargo.lock``
so nested ``--locked`` invocations stay hermetic; a dependency bump therefore
stales them. This script discovers every tracked manifest matching that shape,
so the check, refresh, and prefetch targets stay in step as fixtures are added,
then runs ``cargo metadata --locked`` per manifest and fails with the manifest
path and Cargo output when the lockfile is stale. The prefetch mode instead runs
``cargo fetch --locked`` per manifest, warming ``~/.cargo/registry`` so a later
``--offline`` build of the same lockfile resolves without a network, and closes
the run with one bounded machine-readable metrics record on the stream that
matches the outcome (see :func:`fixture_lockfile_reporting.print_prefetch_metrics`).

Usage: ``python3 scripts/check_fixture_lockfiles.py [--refresh|--fetch] [--list]``.

Exit codes: 0 when every committed fixture lockfile resolves and every prefetch
succeeds; 1 when a lockfile is stale, a prefetch fails, or no fixture was
discovered. The published-GPUI fixtures resolve against staged or crates.io
artefacts, so they are validated by their own ``make check-published-gpui`` and
``make e2e-published-gpui`` targets.
"""

import argparse
import sys
import time
import typing as typ
from pathlib import Path

from fixture_lockfile_alignment import (
    check_alignment_or_failure,
    is_seed_excluded,
)
from fixture_lockfile_cargo import (
    cargo_fetch_command,
    cargo_metadata_command,
    fetch_fixture_dependencies_or_failure,
    refresh_lockfile,
    run_cargo_metadata,
    seed_and_resolve,
)
from fixture_lockfile_discovery import discover_fixture_manifests
from fixture_lockfile_reporting import (
    FixtureLockfileError,
    GateMode,
    fetch_failure_message,
    print_check_summary,
    print_failures,
    print_fetch_summary,
    print_prefetch_metrics,
    print_refresh_summary,
    refresh_failure_message,
    stale_failure_message,
)

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    import subprocess


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    """Parse command-line arguments."""
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument(
        "--refresh",
        action="store_true",
        help="regenerate each discovered lockfile instead of only validating it",
    )
    mode.add_argument(
        "--fetch",
        action="store_true",
        help=(
            "download each discovered lockfile's dependencies into the local "
            "Cargo registry cache instead of validating the lockfile"
        ),
    )
    parser.add_argument("--list", action="store_true", help="list the manifests")
    return parser.parse_args(argv)


def collect_fixture_results(
    root: Path,
    manifests: list[Path],
    operation: cabc.Callable[[Path], subprocess.CompletedProcess[str]],
    prepare: cabc.Callable[[Path, Path], subprocess.CompletedProcess[str] | None]
    | None = None,
) -> list[tuple[Path, subprocess.CompletedProcess[str]]]:
    """Return the failing results of *operation* over *manifests*.

    Each manifest is optionally prepared (refresh mode regenerates its lockfile
    first), then run through *operation*: locked ``cargo metadata`` for the
    check and refresh modes, locked ``cargo fetch`` for the prefetch mode. The
    loop never stops at the first failure so one gate run reports every fixture.

    A preparation's own result is inspected rather than discarded. The
    preparation restores the fixture's committed lockfile when it fails, so
    ``operation`` would then validate a lockfile the fixture still resolves:
    its pass would be a pass over the artefact, not over the refresh that was
    asked for, and the failure that caused the restoration would go unreported.
    A failed preparation is therefore that fixture's failure, and the fixture
    is not validated at all; ``None`` means no preparation was performed.

    Parameters
    ----------
    root : Path
        The repository root, passed to *prepare*.
    manifests : list[Path]
        The fixtures to visit.
    operation : cabc.Callable[[Path], subprocess.CompletedProcess[str]]
        The gate operation to run over each prepared manifest.
    prepare : cabc.Callable[
            [Path, Path], subprocess.CompletedProcess[str] | None
        ]
        | None
        Optional per-manifest step returning its result, or None for a mode
        that prepares nothing.

    Returns
    -------
    list[tuple[Path, subprocess.CompletedProcess[str]]]
        Every ``(manifest, result)`` pair whose operation failed.
    """
    failures: list[tuple[Path, subprocess.CompletedProcess[str]]] = []
    for manifest in manifests:
        if prepare is not None:
            prepared = prepare(root, manifest)
            if prepared is not None and prepared.returncode != 0:
                failures.append((manifest, prepared))
                continue
        result = operation(manifest)
        if result.returncode != 0:
            failures.append((manifest, result))
    return failures


def _is_string_argv(argv: object) -> bool:
    """Return whether *argv* is a non-empty command whose parts are all strings.

    A ``CompletedProcess`` carries whatever ``args`` it was built with, so the
    report can only quote an argv that really is a command: an empty list, a
    string, or a list holding a non-string part names no invocation to rerun,
    and the report falls back to the mode's own command instead.

    Returns
    -------
    bool
        True when *argv* is a usable command line.
    """
    return (
        isinstance(argv, list)
        and bool(argv)
        and all(isinstance(item, str) for item in argv)
    )


def _failing_command(
    mode: GateMode,
    manifest: Path,
    result: subprocess.CompletedProcess[str],
) -> list[str]:
    """Return the Cargo argv that produced *result*, for the failure report.

    The argv *result* carries is preferred over the mode's validating command,
    because a failed preparation's argv is the resolving invocation: quoting
    the validating argv instead would send the reader to a command that
    succeeds and hide the failed one. A result that carries no argv of its own
    falls back to the command the mode would have run.

    Parameters
    ----------
    mode : GateMode
        The mode whose command is the fallback.
    manifest : Path
        The fixture whose operation failed.
    result : subprocess.CompletedProcess[str]
        The failing invocation.

    Returns
    -------
    list[str]
        The argv to quote in the failure report.
    """
    argv = result.args
    if _is_string_argv(argv):
        return argv
    return mode.command(manifest)


def _report_fixture_operation(
    root: Path,
    manifests: list[Path],
    mode: GateMode,
) -> int:
    """Collect, report, and summarize one gate operation over *manifests*.

    The clock runs around the whole operation — every fixture, not the first —
    so the metrics record a mode asks for describes the complete run, and only
    a mode that asks for a record reads the clock at all. A failing fixture
    never ends the loop, so the counts it reports are always the full set the
    run visited.

    Returns
    -------
    int
        The exit code: ``0`` when every fixture passed, otherwise ``1``.
    """
    report_metrics = mode.report_metrics
    started = mode.clock() if report_metrics is not None else None
    results = collect_fixture_results(
        root, manifests, mode.operation, prepare=mode.prepare
    )
    failures = [
        mode.failure_message(
            manifest.relative_to(root),
            _failing_command(mode, manifest, result),
            result.stdout,
            result.stderr,
        )
        for manifest, result in results
    ]
    if failures:
        print_failures(failures)
    mode.print_summary(len(manifests), len(failures))
    if report_metrics is not None and started is not None:
        report_metrics(
            len(manifests), len(failures), round((mode.clock() - started) * 1000)
        )
    return 1 if failures else 0


def seed_fixture_lockfile(
    root: Path, manifest: Path
) -> subprocess.CompletedProcess[str]:
    """Prepare *manifest*'s lockfile for a refresh, then re-resolve it.

    The seed makes the fixture's graph a subset of the workspace's, which is
    what the alignment requirement asks of it. A fixture in
    :data:`~fixture_lockfile_alignment.SEED_EXCLUDED_FIXTURES` is re-resolved
    from its own lockfile instead, because its graph is meant to be different.

    Returns
    -------
    subprocess.CompletedProcess[str]
        The resolving invocation's result.
    """
    if is_seed_excluded(manifest, root):
        return refresh_lockfile(manifest)
    return seed_and_resolve(root / "Cargo.lock", manifest, refresh_lockfile)


def check_fixtures(root: Path, manifests: list[Path]) -> int:
    """Validate every fixture lockfile, returning the process exit code.

    Validation has two parts: ``cargo metadata --locked`` proves each lockfile
    still resolves its manifest, and the alignment pass proves each resolved
    graph stays inside the workspace's, so the nested build can reuse the
    workspace's compiled units instead of rebuilding a second set.

    Returns
    -------
    int
        ``0`` when every lockfile resolves and every graph is aligned,
        otherwise ``1``.
    """
    return _report_fixture_operation(
        root,
        manifests,
        GateMode(
            stale_failure_message,
            print_check_summary,
            cargo_metadata_command,
            run_cargo_metadata,
        ),
    ) or check_alignment_or_failure(root)


def refresh_fixtures(root: Path, manifests: list[Path]) -> int:
    """Regenerate every fixture lockfile, returning the process exit code.

    Each lockfile starts from the root ``Cargo.lock``, so Cargo keeps the
    versions the workspace already selected and adds only what the fixture
    needs; see :func:`fixture_lockfile_cargo.seed_and_resolve` for why the
    seed is what keeps a refresh aligned.

    Fixtures in
    :data:`fixture_lockfile_alignment.SEED_EXCLUDED_FIXTURES` are re-resolved
    without the seed: they exist to resolve outside the workspace graph, so
    seeding would overwrite the very resolution they test.

    Returns
    -------
    int
        ``0`` when every refreshed lockfile still resolves, otherwise ``1``.
    """
    return _report_fixture_operation(
        root,
        manifests,
        GateMode(
            refresh_failure_message,
            print_refresh_summary,
            cargo_metadata_command,
            run_cargo_metadata,
            seed_fixture_lockfile,
        ),
    )


def fetch_fixtures(
    root: Path,
    manifests: list[Path],
    clock: cabc.Callable[[], float] = time.monotonic,
) -> int:
    """Prefetch every fixture's locked dependencies, returning the exit code.

    The run always closes with exactly one machine-readable metrics record,
    whether every fixture downloaded, some failed, or Cargo could not be
    started, on the stream matching the outcome; the human-readable reports and
    summaries are unchanged. *clock* is the monotonic clock that record's
    duration is measured with.

    Returns
    -------
    int
        The exit code: ``0`` when every prefetch succeeded, otherwise ``1``.

    Examples
    --------
    Prefetch every discovered fixture, as ``--fetch`` does; a clean run
    returns ``0`` and any failed ``cargo fetch`` returns ``1``::

        fetch_fixtures(root, discover_fixture_manifests(root))
    """
    return _report_fixture_operation(
        root,
        manifests,
        GateMode(
            fetch_failure_message,
            print_fetch_summary,
            cargo_fetch_command,
            fetch_fixture_dependencies_or_failure,
            report_metrics=print_prefetch_metrics,
            clock=clock,
        ),
    )


def main(argv: list[str] | None = None) -> int:
    """Run the fixture-lockfile gate."""
    args = parse_args(argv)
    root = Path(__file__).resolve().parents[1]
    try:
        manifests = discover_fixture_manifests(root)
    except FixtureLockfileError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    if args.list:
        for manifest in manifests:
            # One spelling per manifest on every platform, so the listing can
            # be diffed or fed back to a tool that expects repository paths.
            print(manifest.relative_to(root).as_posix())
        return 0
    if args.fetch:
        return fetch_fixtures(root, manifests)
    if args.refresh:
        return refresh_fixtures(root, manifests)
    return check_fixtures(root, manifests)


if __name__ == "__main__":
    sys.exit(main())
