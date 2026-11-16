#!/usr/bin/env python3
"""
Resolve the crates a Cargo manifest declares, including inherited aliases.

The example and prelude guards both ask one question of a manifest: does this
crate depend on a given crate, under any spelling Cargo accepts? A dependency
can be declared under a key that differs from the crate it names
(``runtime = { package = "rstest-bdd" }``), in a target-specific table such as
``[target.'cfg(unix)'.dependencies]``, or as an entry inherited from the
workspace (``runtime.workspace = true``). The last form is why the workspace
manifest is read too: the inheriting manifest names only its local key, and
only ``[workspace.dependencies]`` says which crate that key resolves to.

Nothing here parses Rust or judges a manifest; it answers what a manifest
declares so a caller can compare that against the crates it forbids.
"""

import tomllib
import typing as typ

DEPENDENCY_TABLES = ("dependencies", "dev-dependencies", "build-dependencies")

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    from pathlib import Path


def dependency_aliases(root: Path) -> dict[str, str]:
    """
    Return the crate each workspace dependency key resolves to.

    Parameters
    ----------
    root : Path
        The workspace root, holding the manifest with the
        ``[workspace.dependencies]`` table.

    Returns
    -------
    dict[str, str]
        Each workspace dependency key mapped to the crate it declares, which
        differs from the key when the entry renames it with ``package``. A
        missing or unparsable manifest yields no aliases rather than raising:
        an unresolvable inherited entry is then reported under its local key,
        and inventing a crate from nothing would be worse than under-reporting
        a dependency the inheriting manifest never names.
    """
    manifest = root / "Cargo.toml"
    try:
        data = tomllib.loads(manifest.read_text(encoding="utf-8"))
    except OSError, tomllib.TOMLDecodeError:
        return {}
    return {
        name: _declared_name(name, spec)
        for name, spec in _workspace_dependency_table(data).items()
    }


def _workspace_dependency_table(data: object) -> dict[str, object]:
    """Return the ``[workspace.dependencies]`` table, or an empty one."""
    workspace = data.get("workspace") if isinstance(data, dict) else None
    table = workspace.get("dependencies") if isinstance(workspace, dict) else None
    return table if isinstance(table, dict) else {}


def _declared_name(name: str, spec: object) -> str:
    """Return the crate *spec* declares, following a ``package`` rename."""
    package = spec.get("package") if isinstance(spec, dict) else None
    return package if isinstance(package, str) else name


def dependency_names(
    specs: dict[str, object], aliases: dict[str, str]
) -> cabc.Iterator[str]:
    """
    Yield every crate one dependency table declares, by every spelling.

    Parameters
    ----------
    specs : dict[str, object]
        The contents of one dependency table, mapping each dependency's key to
        its specification.
    aliases : dict[str, str]
        The workspace's dependency aliases, as returned by
        :func:`dependency_aliases`, used to resolve an inherited entry.

    Yields
    ------
    str
        Each dependency's key, and the crate it resolves to when that differs:
        ``macros = { package = "rstest-bdd-macros" }`` yields both spellings,
        and ``runtime.workspace = true`` yields the workspace's
        ``runtime = { package = "rstest-bdd" }`` as ``rstest-bdd``.
    """
    for name, spec in specs.items():
        yield name
        yield from _resolved_names(name, spec, aliases)


def _resolved_names(
    name: str, spec: object, aliases: dict[str, str]
) -> cabc.Iterator[str]:
    """Yield the crate *spec* resolves to when its key does not name it."""
    if not isinstance(spec, dict):
        return
    package = spec.get("package")
    if isinstance(package, str):
        yield package
        return
    if spec.get("workspace") is True and name in aliases:
        yield aliases[name]


def dependency_crates(
    table: dict[str, object], aliases: dict[str, str]
) -> cabc.Iterator[str]:
    """
    Yield every crate a parsed manifest subtree depends on.

    The walk recurses, so a dependency declared in a target-specific table such
    as ``[target.'cfg(unix)'.dependencies]`` is found as well as one in a
    top-level table.

    Parameters
    ----------
    table : dict[str, object]
        A parsed TOML table, which may hold nested tables.
    aliases : dict[str, str]
        The workspace's dependency aliases, used to resolve inherited entries.

    Yields
    ------
    str
        Each dependency's key, and the crate it resolves to when that differs.
    """
    for key, value in table.items():
        if not isinstance(value, dict):
            continue
        if key in DEPENDENCY_TABLES:
            yield from dependency_names(value, aliases)
        else:
            yield from dependency_crates(value, aliases)


def declared_dependencies(manifest: Path, aliases: dict[str, str]) -> set[str]:
    """
    Return every crate *manifest* depends on, by every spelling it uses.

    Parameters
    ----------
    manifest : Path
        A crate manifest to parse.
    aliases : dict[str, str]
        The workspace's dependency aliases, used to resolve inherited entries.

    Returns
    -------
    set[str]
        Dependency keys and the crates they resolve to, from the normal,
        development and build tables and from any target-specific table. A
        manifest that is not valid TOML propagates ``tomllib``'s decode error
        to the caller, which reports it against the file.
    """
    data = tomllib.loads(manifest.read_text(encoding="utf-8"))
    return set(dependency_crates(data, aliases))
