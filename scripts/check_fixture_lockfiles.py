#!/usr/bin/env python3
"""Validate and refresh the standalone fixture lockfiles.

Standalone fixture crates opt out of the workspace with a ``[workspace]``
stanza, depend on first-party crates through ``path =`` dependencies, and
commit their own ``Cargo.lock`` so nested ``--locked`` Cargo invocations stay
hermetic. A dependency bump that changes the workspace resolution therefore
stales those lockfiles and the nested runs fail.

This script is the single authority for that fixture set: it discovers every
tracked manifest matching that shape, so the check and the refresh targets stay
in step as fixtures are added. For every selected manifest it runs
``cargo metadata --locked --format-version 1 --manifest-path <manifest>`` and
fails with the manifest path and Cargo output when the lockfile is stale.

Usage: ``python3 scripts/check_fixture_lockfiles.py [--refresh] [--list]``.

Exit codes: 0 when every committed fixture lockfile resolves; 1 when a lockfile
is stale or no fixture was discovered. The published-GPUI fixtures resolve
against staged or crates.io artefacts, so they are validated by their own
``make check-published-gpui`` and ``make e2e-published-gpui`` targets.
"""

import argparse
import subprocess  # ruff: ignore[suspicious-subprocess-import] - the gate invokes the trusted local cargo executable.
import sys
import typing as typ
from pathlib import Path

from fixture_lockfile_reporting import (
    print_check_summary,
    print_failures,
    print_refresh_summary,
    refresh_failure_message,
    stale_failure_message,
)

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: Directory names the scan never descends into: build output, tool caches and
#: editor state can all hold generated manifests that are not fixtures.
EXCLUDED_DIRECTORIES = frozenset({".git", ".vtcode", "target", "node_modules", ".venv"})

#: Manifests that opt out of the workspace but resolve against artefacts staged
#: under ``target/`` by ``make stage-published-gpui-e2e``. They have no lockfile
#: to validate until that staging runs, so they stay on their own targets.
STAGED_FIXTURES = frozenset({"tests/fixtures/published-gpui-e2e"})

CARGO = "cargo"
METADATA_FORMAT_VERSION = "1"


class FixtureLockfileError(RuntimeError):
    """A fixture lockfile is stale, or no fixture manifest was discovered."""

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


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    """Parse command-line arguments."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--refresh",
        action="store_true",
        help="regenerate each discovered lockfile instead of only validating it",
    )
    parser.add_argument("--list", action="store_true", help="list the manifests")
    return parser.parse_args(argv)


def iter_cargo_manifests(root: Path) -> cabc.Iterator[Path]:
    """Yield every ``Cargo.toml`` beneath *root*, pruning excluded directories.

    Walking manually (rather than ``Path.rglob``) keeps build output such as
    ``target/`` and nested scratch copies out of the discovery set.

    Yields
    ------
    Path
        The next manifest beneath *root*, outside the excluded directories.
    """
    stack = [root]
    while stack:
        directory = stack.pop()
        try:
            entries = sorted(directory.iterdir())
        except OSError:
            continue
        for entry in entries:
            if entry.is_dir() and entry.name not in EXCLUDED_DIRECTORIES:
                stack.append(entry)
            elif entry.name == "Cargo.toml" and entry.is_file():
                yield entry


def is_standalone_workspace(manifest: Path) -> bool:
    """Return whether *manifest* opts out of the root workspace.

    A fixture excludes itself from the enclosing workspace by declaring its own
    ``[workspace]`` section. The stanza usually carries no members, so the check
    looks for the section rather than for a member list.

    Parameters
    ----------
    manifest : Path
        The manifest under test.

    Returns
    -------
    bool
        True when the manifest declares its own ``[workspace]`` section.
    """
    try:
        text = manifest.read_text(encoding="utf-8")
    except OSError:
        return False
    return "[workspace]" in text


def has_path_dependency(manifest: Path) -> bool:
    """Return whether *manifest* declares at least one local ``path =`` source.

    Parameters
    ----------
    manifest : Path
        The manifest under test.

    Returns
    -------
    bool
        True when at least one dependency resolves from the local filesystem.
    """
    try:
        text = manifest.read_text(encoding="utf-8")
    except OSError:
        return False
    return "path = " in text


def is_staged_fixture(manifest: Path, root: Path) -> bool:
    """Return whether *manifest* resolves against ``target/`` staged artefacts.

    The published-GPUI end-to-end fixture patches crates.io dependencies onto
    ``target/published-gpui-e2e/`` package extractions that only exist after
    ``make stage-published-gpui-e2e``. It therefore has no resolvable graph
    until staging runs and is validated by the dedicated targets instead.

    Parameters
    ----------
    manifest : Path
        The manifest under test.
    root : Path
        The repository root the manifest lives beneath.

    Returns
    -------
    bool
        True when the manifest belongs to a staged fixture directory.
    """
    try:
        return manifest.parent.relative_to(root).as_posix() in STAGED_FIXTURES
    except ValueError:
        return False


def is_workspace_root(manifest: Path, root: Path) -> bool:
    """Return whether *manifest* is the root workspace manifest itself.

    The root manifest resolves through the workspace resolver against the root
    lockfile, which the ordinary workspace build already validates. The fixture
    gate covers only the crates that opted out of that resolver.

    Parameters
    ----------
    manifest : Path
        The manifest under test.
    root : Path
        The repository root the manifest lives beneath.

    Returns
    -------
    bool
        True when *manifest* is the workspace root manifest.
    """
    return manifest == root / "Cargo.toml"


def discover_fixture_manifests(root: Path) -> list[Path]:
    """Return every standalone fixture manifest beneath *root*, sorted.

    A manifest qualifies when it opts out of the workspace with ``[workspace]``,
    uses a local ``path =`` dependency, and commits a sibling ``Cargo.lock``.

    Parameters
    ----------
    root : Path
        The repository root to scan.

    Returns
    -------
    list[Path]
        The sorted authoritative fixture manifests.

    Raises
    ------
    FixtureLockfileError
        No fixture manifest was found, so the gate's contract is broken.
    """
    manifests = [
        path
        for path in iter_cargo_manifests(root)
        if is_standalone_workspace(path)
        and has_path_dependency(path)
        and (path.parent / "Cargo.lock").is_file()
        and not is_staged_fixture(path, root)
        and not is_workspace_root(path, root)
    ]
    if not manifests:
        raise FixtureLockfileError(FixtureLockfileError.no_manifests_message())
    return sorted(manifests)


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


def run_cargo_metadata(manifest: Path) -> subprocess.CompletedProcess[str]:
    """Run locked ``cargo metadata`` for *manifest* and return the result.

    ``cargo metadata --locked`` is the authoritative lockfile check: Cargo
    re-resolves the manifest against the committed lockfile and fails when the
    two disagree, which is exactly the staleness this gate exists to catch.

    Parameters
    ----------
    manifest : Path
        The standalone fixture manifest to resolve.

    Returns
    -------
    subprocess.CompletedProcess[str]
        The completed Cargo invocation with captured output.

    Raises
    ------
    FixtureLockfileError
        The Cargo executable could not be started at all.
    """
    try:
        return subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - argv carries a fixed command plus the manifest path; no shell.
            cargo_metadata_command(manifest),
            capture_output=True,
            text=True,
            check=False,
        )
    except OSError as error:
        raise FixtureLockfileError(
            FixtureLockfileError.cargo_unavailable_message(CARGO, manifest, error)
        ) from error


def refresh_lockfile(manifest: Path) -> subprocess.CompletedProcess[str]:
    """Regenerate the lockfile for *manifest* through Cargo.

    Cargo's own lockfile writer produces the committed artefact; the script
    never hand-edits package records or checksums.

    Parameters
    ----------
    manifest : Path
        The standalone fixture manifest whose lockfile should be regenerated.

    Returns
    -------
    subprocess.CompletedProcess[str]
        The completed Cargo invocation with captured output.

    Raises
    ------
    FixtureLockfileError
        The Cargo executable could not be started at all.
    """
    try:
        return subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - argv carries a fixed command plus the manifest path; no shell.
            [CARGO, "generate-lockfile", "--manifest-path", str(manifest)],
            capture_output=True,
            text=True,
            check=False,
        )
    except OSError as error:
        raise FixtureLockfileError(
            FixtureLockfileError.cargo_unavailable_message(CARGO, manifest, error)
        ) from error


def check_fixtures(root: Path, manifests: list[Path]) -> int:
    """Validate every fixture lockfile, returning the process exit code."""
    failures: list[str] = []
    for manifest in manifests:
        display = manifest.relative_to(root)
        result = run_cargo_metadata(manifest)
        if result.returncode != 0:
            failures.append(
                stale_failure_message(
                    display,
                    cargo_metadata_command(manifest),
                    result.stdout,
                    result.stderr,
                )
            )
    if failures:
        print_failures(failures)
        print_check_summary(len(manifests), len(failures))
        return 1
    print_check_summary(len(manifests), 0)
    return 0


def refresh_fixtures(root: Path, manifests: list[Path]) -> int:
    """Regenerate every fixture lockfile, returning the process exit code."""
    failures: list[str] = []
    for manifest in manifests:
        display = manifest.relative_to(root)
        refresh_lockfile(manifest)
        result = run_cargo_metadata(manifest)
        if result.returncode != 0:
            failures.append(
                refresh_failure_message(
                    display,
                    cargo_metadata_command(manifest),
                    result.stdout,
                    result.stderr,
                )
            )
    if failures:
        print_failures(failures)
        print_refresh_summary(len(manifests), len(failures))
        return 1
    print_refresh_summary(len(manifests), 0)
    return 0


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
    if args.refresh:
        return refresh_fixtures(root, manifests)
    return check_fixtures(root, manifests)


if __name__ == "__main__":
    sys.exit(main())
