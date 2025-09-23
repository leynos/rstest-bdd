#!/usr/bin/env -S uv run python
# /// script
# requires-python = ">=3.11"
# dependencies = [
#     "tomlkit",
# ]
# ///
"""Utility helpers for adjusting manifests during publish checks."""
from __future__ import annotations

import argparse
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

from tomlkit import TOMLDocument, dumps, inline_table, parse
from tomlkit.items import InlineTable, Table


@dataclass(frozen=True)
class DependencyPatch:
    """Describe how a dependency should be rewritten for publish checks."""

    section: str
    name: str
    path: str


REPLACEMENTS: dict[str, tuple[DependencyPatch, ...]] = {
    "rstest-bdd-macros": (
        DependencyPatch("dependencies", "rstest-bdd-patterns", "../rstest-bdd-patterns"),
        DependencyPatch("dev-dependencies", "rstest-bdd", "../rstest-bdd"),
    ),
    "rstest-bdd": (
        DependencyPatch("dependencies", "rstest-bdd-patterns", "../rstest-bdd-patterns"),
        DependencyPatch("dev-dependencies", "rstest-bdd-macros", "../rstest-bdd-macros"),
    ),
    "cargo-bdd": (
        DependencyPatch("dependencies", "rstest-bdd", "../rstest-bdd"),
    ),
}


def apply_replacements(crate: str, manifest: Path, version: str) -> None:
    """Rewrite workspace dependencies to point at packaged versions.

    Parameters
    ----------
    crate : str
        Name of the crate whose manifest should be altered.
    manifest : Path
        Path to the `Cargo.toml` file that will be rewritten in place.
    version : str
        Version string applied to patched dependency entries.

    Raises
    ------
    SystemExit
        Raised when *crate* does not have a configured replacement set.

    Examples
    --------
    >>> from pathlib import Path
    >>> tmp = Path('Cargo.toml')
    >>> _ = tmp.write_text(
    ...     '[dependencies]\n'
    ...     'rstest-bdd-patterns = { path = "../rstest-bdd-patterns" }\n'
    ...     '[dev-dependencies]\n'
    ...     'rstest-bdd-macros = { path = "../rstest-bdd-macros" }'
    ... )
    >>> apply_replacements('rstest-bdd', tmp, '1.2.3')
    >>> 'version = "1.2.3"' in tmp.read_text()
    True
    """
    document = parse(manifest.read_text(encoding="utf-8"))
    patches = REPLACEMENTS.get(crate)
    if patches is None:
        raise SystemExit(f"unknown crate {crate!r}")
    for patch in patches:
        update_dependency(document, patch, version, manifest)
    manifest.write_text(dumps(document), encoding="utf-8")


def update_dependency(
    document: TOMLDocument,
    patch: DependencyPatch,
    version: str,
    manifest: Path,
) -> None:
    """Replace a workspace dependency with an inline publish-friendly entry.

    Parameters
    ----------
    document : TOMLDocument
        Parsed manifest document that will be mutated in place.
    patch : DependencyPatch
        Replacement metadata describing the dependency to update.
    version : str
        Version string used for the inline dependency.
    manifest : Path
        Path to the manifest used for error reporting.

    Raises
    ------
    SystemExit
        Raised when the manifest is missing the targeted section or dependency.

    Examples
    --------
    >>> from pathlib import Path
    >>> from tomlkit import parse
    >>> doc = parse('[dependencies]\nfoo = { path = "../foo" }')
    >>> patch = DependencyPatch('dependencies', 'foo', '../foo')
    >>> update_dependency(doc, patch, '1.0.0', Path('Cargo.toml'))
    >>> dict(doc['dependencies']['foo'])['version']
    '1.0.0'
    """
    try:
        section = document[patch.section]
    except KeyError as error:
        raise SystemExit(
            f"expected section [{patch.section}] in {manifest}"
        ) from error
    try:
        existing = section[patch.name]
    except KeyError as error:
        raise SystemExit(
            f"expected dependency {patch.name!r} in {manifest}"
        ) from error
    extra_items = extract_existing_items(existing)
    section[patch.name] = build_inline_dependency(extra_items, patch.path, version)


def extract_existing_items(value: object) -> Iterable[tuple[str, object]]:
    """Return preserved dependency metadata from an existing entry.

    Parameters
    ----------
    value : object
        Existing dependency definition, potentially a table or inline table.

    Returns
    -------
    Iterable[tuple[str, object]]
        Key-value pairs that should be retained when rebuilding the entry.

    Examples
    --------
    >>> from tomlkit import parse
    >>> table = parse('[dependencies]\nfoo = { default-features = false }')
    >>> items = extract_existing_items(table['dependencies']['foo'])
    >>> dict(items)
    {'default-features': False}
    """
    if isinstance(value, (Table, InlineTable)):
        return tuple(
            (key, item)
            for key, item in value.items()
            if key not in {"workspace", "path", "version"}
        )
    return ()


def build_inline_dependency(
    extra_items: Iterable[tuple[str, object]],
    path: str,
    version: str,
) -> InlineTable:
    """Construct a normalised inline dependency table.

    Parameters
    ----------
    extra_items : Iterable[tuple[str, object]]
        Additional metadata retained from the previous dependency definition.
    path : str
        Relative path to the dependency crate.
    version : str
        Version string for the dependency.

    Returns
    -------
    InlineTable
        Inline table ready to be inserted into the manifest document.

    Examples
    --------
    >>> inline = build_inline_dependency((), '../foo', '1.0.0')
    >>> dict(inline)
    {'path': '../foo', 'version': '1.0.0'}
    """
    dependency = inline_table()
    dependency["path"] = path
    dependency["version"] = version
    for key, item in extra_items:
        dependency[key] = item
    dependency.trailing_comma = False
    return dependency


def main() -> None:
    """Parse CLI arguments and rewrite the requested manifest.

    Raises
    ------
    SystemExit
        Propagated when argument parsing fails or an unknown crate is given.

    Examples
    --------
    >>> import sys
    >>> from pathlib import Path
    >>> tmp = Path('Cargo.toml')
    >>> _ = tmp.write_text(
    ...     '[dependencies]\n'
    ...     'rstest-bdd-patterns = { path = "../rstest-bdd-patterns" }\n'
    ...     '[dev-dependencies]\n'
    ...     'rstest-bdd-macros = { path = "../rstest-bdd-macros" }'
    ... )
    >>> argv = sys.argv
    >>> sys.argv = [
    ...     'publish_patch.py',
    ...     'rstest-bdd',
    ...     str(tmp),
    ...     '--version',
    ...     '1.2.3',
    ... ]
    >>> try:
    ...     main()
    ... finally:
    ...     sys.argv = argv
    >>> 'version = "1.2.3"' in tmp.read_text()
    True
    """
    parser = argparse.ArgumentParser(
        description="Adjust workspace manifests for publish-check packaging."
    )
    parser.add_argument("crate", choices=sorted(REPLACEMENTS))
    parser.add_argument("manifest", type=Path)
    parser.add_argument("--version", required=True)
    args = parser.parse_args()
    apply_replacements(args.crate, args.manifest, args.version)


if __name__ == "__main__":
    main()
