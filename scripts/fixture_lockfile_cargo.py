#!/usr/bin/env python3
"""Build and run the Cargo invocations the fixture lockfile gate needs.

The gate has three operations over one manifest: validate it with a locked
``cargo metadata``, re-resolve it for a refresh, and prefetch its dependencies
with a locked ``cargo fetch``. Each is a fixed argv plus the manifest path, run
with captured output and a non-raising return code, so a single stale lockfile
is reported rather than aborting the run. Keeping the argv construction and the
invocation together leaves the gate script to decide what to do with the
results, which is the part that differs between the modes.
"""

import shutil
import subprocess  # ruff: ignore[suspicious-subprocess-import] - builds and runs a fixed cargo argv; see run_cargo_command.
import typing as typ

from fixture_lockfile_reporting import FixtureLockfileError

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    from pathlib import Path

CARGO = "cargo"
METADATA_FORMAT_VERSION = "1"


def restore_lockfile(lockfile: Path, snapshot: bytes | None) -> None:
    """Restore *lockfile* from *snapshot*, or remove it when there was none.

    Parameters
    ----------
    lockfile : Path
        The fixture lockfile the seed was copied over.
    snapshot : bytes | None
        The bytes read before the copy, or None when the fixture committed no
        lockfile for the copy to have replaced.
    """
    if snapshot is None:
        lockfile.unlink(missing_ok=True)
    else:
        lockfile.write_bytes(snapshot)


def seed_and_resolve(
    root_lock: Path,
    manifest: Path,
    resolve: cabc.Callable[[Path], subprocess.CompletedProcess[str]],
) -> subprocess.CompletedProcess[str]:
    """Seed *manifest*'s lockfile from *root_lock*, then re-resolve it.

    Seeding pins every shared crate to the version the workspace selected, so
    Cargo has only the fixture's own additions left to make. Without it the
    resolver picks the highest version each requirement allows, which is not
    the workspace's choice: a refresh of this repository moved several crates
    to newer patch releases and left all three fixtures outside the workspace
    graph. *resolve* is injected so the seeding can be tested without Cargo.

    The seed is kept only when the resolution succeeds. A refresh that returns
    non-zero, or that is interrupted while it runs, puts the fixture's own
    lockfile back: the copy is a starting point rather than a resolution, so
    leaving it in place would let the next locked validation compare the
    fixture's manifest against the workspace's lockfile and pass over a fixture
    that cannot be refreshed. The restoration therefore belongs to a
    ``finally``, which also covers the ``KeyboardInterrupt`` a cancelled run
    raises.

    Parameters
    ----------
    root_lock : Path
        The workspace lockfile to seed from.
    manifest : Path
        The fixture manifest whose sibling ``Cargo.lock`` is rewritten.
    resolve : cabc.Callable[[Path], subprocess.CompletedProcess[str]]
        The resolver to run over *manifest* after seeding.

    Returns
    -------
    subprocess.CompletedProcess[str]
        The resolver's result.

    Examples
    --------
    A refresh target seeds from the root lock and lets Cargo add only what the
    fixture needs::

        seed_and_resolve(Path("Cargo.lock"), manifest, refresh_lockfile)
    """
    lockfile = manifest.parent / "Cargo.lock"
    snapshot = lockfile.read_bytes() if lockfile.is_file() else None
    shutil.copyfile(root_lock, lockfile)
    resolved = False
    try:
        result = resolve(manifest)
        resolved = result.returncode == 0
        return result
    finally:
        if not resolved:
            restore_lockfile(lockfile, snapshot)


def cargo_metadata_command(manifest: Path) -> list[str]:
    """Build the locked ``cargo metadata`` command for *manifest*.

    Returns
    -------
    list[str]
        The locked ``cargo metadata`` argv for *manifest*.
    """
    return [
        CARGO,
        "metadata",
        "--locked",
        "--format-version",
        METADATA_FORMAT_VERSION,
        "--manifest-path",
        str(manifest),
    ]


def cargo_resolve_command(manifest: Path) -> list[str]:
    """Build the command that re-resolves *manifest*'s lockfile.

    ``cargo metadata`` without ``--locked`` resolves the manifest and writes any
    lockfile change, but keeps the versions an existing lockfile already pins
    instead of upgrading them. ``cargo generate-lockfile`` upgrades, which is
    why it is not used here; see :func:`seed_and_resolve`.

    Returns
    -------
    list[str]
        The resolving ``cargo metadata`` argv for *manifest*.
    """
    return [
        CARGO,
        "metadata",
        "--format-version",
        METADATA_FORMAT_VERSION,
        "--manifest-path",
        str(manifest),
    ]


def cargo_fetch_command(manifest: Path) -> list[str]:
    """Build the locked ``cargo fetch`` command for *manifest*.

    Returns
    -------
    list[str]
        The locked ``cargo fetch`` argv for *manifest*.

    Examples
    --------
    >>> cargo_fetch_command(Path("fixtures/minimal/Cargo.toml"))
    ['cargo', 'fetch', '--locked', '--manifest-path', 'fixtures/minimal/Cargo.toml']
    """
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
    return run_cargo_command(cargo_resolve_command(manifest), manifest)


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

    Examples
    --------
    Prefetch the dependencies of a single fixture manifest::

        fetch_fixture_dependencies(
            Path("crates/cargo-bdd/tests/fixtures/minimal/Cargo.toml")
        )

    The completed invocation carries ``returncode == 0`` once every locked
    crate is cached; a non-zero code marks that fixture as a failed prefetch.
    """
    return run_cargo_command(cargo_fetch_command(manifest), manifest)


def fetch_fixture_dependencies_or_failure(
    manifest: Path,
) -> subprocess.CompletedProcess[str]:
    """Prefetch *manifest*, turning an unstartable Cargo into a failed result.

    ``cargo fetch`` is one fixture's work, so a Cargo that cannot be started
    must fail that fixture rather than the whole run: the prefetch visits every
    fixture and closes with exactly one metrics record either way. The launch
    error therefore becomes a failed synthetic invocation carrying the same
    message, which the shared report renders like any other prefetch failure.
    The check and refresh modes keep :func:`run_cargo_command`'s raising
    contract, because a lockfile cannot be validated without Cargo at all.

    Returns
    -------
    subprocess.CompletedProcess[str]
        The completed invocation, or a failed synthetic one when Cargo could
        not be started.
    """
    try:
        return fetch_fixture_dependencies(manifest)
    except FixtureLockfileError as error:
        return subprocess.CompletedProcess(
            args=cargo_fetch_command(manifest),
            returncode=1,
            stdout="",
            stderr=f"{error}\n",
        )
