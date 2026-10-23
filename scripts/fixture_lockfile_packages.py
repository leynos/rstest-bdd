#!/usr/bin/env python3
"""Read the resolved packages out of a Cargo lockfile or manifest.

Both sides of the alignment comparison are the same kind of document — a
lockfile with a ``[[package]]`` array — so reading them belongs in one place
rather than being split across the module that owns the policy and the module
that owns the Cargo invocations.

A package is identified the way Cargo identifies one: by ``(name, version,
source)``. ``source`` is ``None`` for a path dependency and a registry or git
URL otherwise, so two packages of the same name and version from different
sources stay distinct. Comparing names alone would call a path override of a
registry crate the same package and miss exactly the fork this gate exists to
find.
"""

import tomllib
import typing as typ

from fixture_lockfile_reporting import FixtureLockfileError

if typ.TYPE_CHECKING:
    from pathlib import Path

#: The lockfile table whose entries carry ``name``, ``version`` and ``source``.
PACKAGE_TABLE = "package"

#: A resolved package, identified the way Cargo identifies one.
type PackageKey = tuple[str, str, str | None]


class LockfileUnreadableError(FixtureLockfileError):
    """A lockfile could not be parsed as TOML.

    Raised rather than skipped: an unreadable lockfile has no resolved graph to
    compare, and treating it as empty would let the gate pass over the very
    fixture it exists to check.

    Parameters
    ----------
    path : Path
        The lockfile that could not be read.

    See Also
    --------
    read_lockfile_packages : The reading that raises this.
    """

    def __init__(self, path: Path) -> None:
        """Build the message from the parameters documented on the class."""
        super().__init__(f"cannot read lockfile {path.as_posix()} as TOML")


def package_key(entry: dict[str, object]) -> PackageKey | None:
    """Return one lockfile entry's identity, or None when it is not a package.

    Parameters
    ----------
    entry : dict[str, object]
        One element of a lockfile's ``[[package]]`` array.

    Returns
    -------
    PackageKey | None
        The identity, or None when the entry carries no name or version.
    """
    name = entry.get("name")
    version = entry.get("version")
    if not isinstance(name, str) or not isinstance(version, str):
        return None
    source = entry.get("source")
    return (name, version, source if isinstance(source, str) else None)


def read_lockfile_packages(path: Path) -> set[PackageKey]:
    """Return every package identity a lockfile resolves.

    Parameters
    ----------
    path : Path
        The lockfile to read.

    Returns
    -------
    set[PackageKey]
        The ``(name, version, source)`` of every resolved package.

    Raises
    ------
    LockfileUnreadableError
        The file is missing, unreadable, or not valid TOML.

    Examples
    --------
    The root lockfile resolves ``serde`` from crates.io, so its entry appears
    with that registry source::

        packages = read_lockfile_packages(Path("Cargo.lock"))
        registry = "registry+https://github.com/rust-lang/crates.io-index"
        assert ("serde", "1.0.228", registry) in packages
    """
    try:
        with path.open("rb") as handle:
            document = tomllib.load(handle)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise LockfileUnreadableError(path) from error
    entries = document.get(PACKAGE_TABLE, [])
    if not isinstance(entries, list):
        raise LockfileUnreadableError(path)
    keys = (package_key(entry) for entry in entries if isinstance(entry, dict))
    return {key for key in keys if key is not None}


def own_package(manifest: Path) -> PackageKey | None:
    """Return the package the fixture manifest itself declares.

    The fixture's own crate appears in its lockfile and cannot appear in the
    workspace lockfile, so it is not drift. Reading the name from the manifest
    keeps the exemption honest: it covers exactly the crate being built, not
    every package whose name happens to be absent from the workspace.

    Parameters
    ----------
    manifest : Path
        The fixture manifest.

    Returns
    -------
    PackageKey | None
        The fixture's own ``(name, version, None)``, or None when the manifest
        does not declare a readable package.

    Raises
    ------
    LockfileUnreadableError
        The manifest is missing, unreadable, or not valid TOML.
    """
    try:
        with manifest.open("rb") as handle:
            document = tomllib.load(handle)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise LockfileUnreadableError(manifest) from error
    package = document.get("package")
    if not isinstance(package, dict):
        return None
    return package_key(package)
