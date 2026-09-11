#!/usr/bin/env python3
"""Discover the standalone fixture manifests the lockfile gate covers.

Standalone fixture crates opt out of the workspace with a ``[workspace]``
stanza, use local ``path =`` dependencies, and commit their own ``Cargo.lock``
so nested ``--locked`` invocations stay hermetic; a dependency bump therefore
stales them. Discovery lives apart from the gate's Cargo plumbing so the
validation gate, the refresh path, and the mutation lane's dependency prefetch
all resolve one fixture set, and so the gate script stays inside the 400-line
module budget.
"""

import typing as typ

from fixture_lockfile_reporting import FixtureLockfileError

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    from pathlib import Path

#: Directory names the scan never descends into: build output, tool caches and
#: editor state can all hold generated manifests that are not fixtures.
EXCLUDED_DIRECTORIES = frozenset({".git", ".vtcode", "target", "node_modules", ".venv"})

#: Manifests that opt out of the workspace but resolve against artefacts staged
#: under ``target/`` by ``make stage-published-gpui-e2e``. They have no lockfile
#: to validate until that staging runs, so they stay on their own targets.
STAGED_FIXTURES = frozenset({"tests/fixtures/published-gpui-e2e"})


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
