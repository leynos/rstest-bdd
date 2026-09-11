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
``--offline`` build of the same lockfile resolves without a network.

Usage: ``python3 scripts/check_fixture_lockfiles.py [--refresh|--fetch] [--list]``.

Exit codes: 0 when every committed fixture lockfile resolves and every prefetch
succeeds; 1 when a lockfile is stale, a prefetch fails, or no fixture was
discovered. The published-GPUI fixtures resolve against staged or crates.io
artefacts, so they are validated by their own ``make check-published-gpui`` and
``make e2e-published-gpui`` targets.
"""

import argparse
import subprocess  # ruff: ignore[suspicious-subprocess-import] - the gate invokes the trusted local cargo executable.
import sys
import typing as typ
from pathlib import Path

from fixture_lockfile_discovery import discover_fixture_manifests
from fixture_lockfile_reporting import (
    FixtureLockfileError,
    GateMode,
    fetch_failure_message,
    print_check_summary,
    print_failures,
    print_fetch_summary,
    print_refresh_summary,
    refresh_failure_message,
    stale_failure_message,
)

if typ.TYPE_CHECKING:
    import collections.abc as cabc

CARGO = "cargo"
METADATA_FORMAT_VERSION = "1"


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


def cargo_metadata_command(manifest: Path) -> list[str]:
    """Build the locked ``cargo metadata`` command for *manifest*."""
    return [
        CARGO,
        "metadata",
        "--locked",
        "--format-version",
        METADATA_FORMAT_VERSION,
        "--manifest-path",
        str(manifest),
    ]


def cargo_refresh_command(manifest: Path) -> list[str]:
    """Build the ``cargo generate-lockfile`` command for *manifest*."""
    return [CARGO, "generate-lockfile", "--manifest-path", str(manifest)]


def cargo_fetch_command(manifest: Path) -> list[str]:
    """Build the locked ``cargo fetch`` command for *manifest*."""
    return [CARGO, "fetch", "--locked", "--manifest-path", str(manifest)]


def run_cargo_command(
    command: list[str], manifest: Path
) -> subprocess.CompletedProcess[str]:
    """Run a Cargo command for *manifest* and return the captured result.

    Every gate invocation uses the same plumbing: a fixed argv (no shell),
    captured output, and a non-raising return code so stale lockfiles are
    reported per fixture instead of aborting the whole run.

    Returns
    -------
    subprocess.CompletedProcess[str]
        The completed invocation with captured output.

    Raises
    ------
    FixtureLockfileError
        Cargo could not be started; the message names it and *manifest*.
    """
    try:
        return subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - argv carries a fixed command plus the manifest path; no shell.
            command, capture_output=True, text=True, check=False
        )
    except OSError as error:
        raise FixtureLockfileError(
            FixtureLockfileError.cargo_unavailable_message(CARGO, manifest, error)
        ) from error


def run_cargo_metadata(manifest: Path) -> subprocess.CompletedProcess[str]:
    """Run locked ``cargo metadata`` for *manifest* and return the result.

    ``cargo metadata --locked`` is the authoritative lockfile check: Cargo
    re-resolves the manifest against the committed lockfile and fails when the
    two disagree, which is exactly the staleness this gate exists to catch.

    Returns
    -------
    subprocess.CompletedProcess[str]
        The completed invocation; errors per :func:`run_cargo_command`.
    """
    return run_cargo_command(cargo_metadata_command(manifest), manifest)


def refresh_lockfile(manifest: Path) -> subprocess.CompletedProcess[str]:
    """Regenerate the lockfile for *manifest* through Cargo.

    Cargo's own lockfile writer produces the committed artefact; the script
    never hand-edits package records or checksums.

    Returns
    -------
    subprocess.CompletedProcess[str]
        The completed invocation; errors per :func:`run_cargo_command`.
    """
    return run_cargo_command(cargo_refresh_command(manifest), manifest)


def fetch_fixture_dependencies(manifest: Path) -> subprocess.CompletedProcess[str]:
    """Download the locked dependencies of *manifest* into the Cargo cache.

    ``cargo fetch --locked`` fills ``~/.cargo/registry`` from the committed
    lockfile without building anything, so a later ``--offline`` invocation of
    the same lockfile resolves from the cache. The mutation lane runs this mode
    because it has no outer online workspace build to warm that cache before its
    nested fixture tests run.

    Returns
    -------
    subprocess.CompletedProcess[str]
        The completed invocation; errors per :func:`run_cargo_command`.
    """
    return run_cargo_command(cargo_fetch_command(manifest), manifest)


def collect_fixture_results(
    manifests: list[Path],
    operation: cabc.Callable[[Path], subprocess.CompletedProcess[str]],
    prepare: cabc.Callable[[Path], object] | None = None,
) -> list[tuple[Path, subprocess.CompletedProcess[str]]]:
    """Return the failing results of *operation* over *manifests*.

    Each manifest is optionally prepared (refresh mode regenerates its lockfile
    first), then run through *operation*: locked ``cargo metadata`` for the
    check and refresh modes, locked ``cargo fetch`` for the prefetch mode. The
    loop never stops at the first failure so one gate run reports every fixture.

    Returns
    -------
    list[tuple[Path, subprocess.CompletedProcess[str]]]
        Every ``(manifest, result)`` pair whose operation failed.
    """
    failures: list[tuple[Path, subprocess.CompletedProcess[str]]] = []
    for manifest in manifests:
        if prepare is not None:
            prepare(manifest)
        result = operation(manifest)
        if result.returncode != 0:
            failures.append((manifest, result))
    return failures


def _report_fixture_operation(
    root: Path,
    manifests: list[Path],
    mode: GateMode,
) -> int:
    """Collect, report, and summarize one gate operation over *manifests*."""
    results = collect_fixture_results(manifests, mode.operation, prepare=mode.prepare)
    failures = [
        mode.failure_message(
            manifest.relative_to(root),
            mode.command(manifest),
            result.stdout,
            result.stderr,
        )
        for manifest, result in results
    ]
    if failures:
        print_failures(failures)
    mode.print_summary(len(manifests), len(failures))
    return 1 if failures else 0


def check_fixtures(root: Path, manifests: list[Path]) -> int:
    """Validate every fixture lockfile, returning the process exit code."""
    return _report_fixture_operation(
        root,
        manifests,
        GateMode(
            stale_failure_message,
            print_check_summary,
            cargo_metadata_command,
            run_cargo_metadata,
        ),
    )


def refresh_fixtures(root: Path, manifests: list[Path]) -> int:
    """Regenerate every fixture lockfile, returning the process exit code."""
    return _report_fixture_operation(
        root,
        manifests,
        GateMode(
            refresh_failure_message,
            print_refresh_summary,
            cargo_metadata_command,
            run_cargo_metadata,
            refresh_lockfile,
        ),
    )


def fetch_fixtures(root: Path, manifests: list[Path]) -> int:
    """Prefetch every fixture's locked dependencies, returning the exit code."""
    return _report_fixture_operation(
        root,
        manifests,
        GateMode(
            fetch_failure_message,
            print_fetch_summary,
            cargo_fetch_command,
            fetch_fixture_dependencies,
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
            print(manifest.relative_to(root))
        return 0
    if args.fetch:
        return fetch_fixtures(root, manifests)
    if args.refresh:
        return refresh_fixtures(root, manifests)
    return check_fixtures(root, manifests)


if __name__ == "__main__":
    sys.exit(main())
