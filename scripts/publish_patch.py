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


@dataclass(frozen=True)
class DependencyConfig:
    """Configuration for dependency replacement operations.

    Parameters
    ----------
    version : str
        Version string applied to rewritten dependency entries.
    include_local_path : bool, default True
        When ``True`` the inline dependency retains its ``path`` attribute so
        publish checks continue to use the locally exported workspace. Disable
        this flag for live publishing so Cargo resolves crates from crates.io.
    """

    version: str
    include_local_path: bool = True


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


def apply_replacements(
    crate: str,
    manifest: Path,
    version: str,
    *,
    include_local_path: bool = True,
) -> None:
    """Rewrite workspace dependencies to point at packaged versions.

    Parameters
    ----------
    crate : str
        Name of the crate whose manifest should be altered.
    manifest : Path
        Path to the `Cargo.toml` file that will be rewritten in place.
    version : str
        Version string applied to patched dependency entries.
    include_local_path : bool, default True
        Retain the relative ``path`` entry alongside the version when updating
        manifests. Publish checks rely on the path so crates can depend on the
        locally exported workspace. Disable this for live publishing so Cargo
        talks to crates.io instead.

    Returns
    -------
    None
        The manifest file is rewritten in place with patched dependencies.

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
    config = DependencyConfig(version=version, include_local_path=include_local_path)
    for patch in patches:
        update_dependency(document, patch, config, manifest)
    manifest.write_text(dumps(document), encoding="utf-8")


def update_dependency(
    document: TOMLDocument,
    patch: DependencyPatch,
    config: DependencyConfig,
    manifest: Path,
) -> None:
    """Replace a workspace dependency with an inline publish-friendly entry.

    Parameters
    ----------
    document : TOMLDocument
        Parsed manifest document that will be mutated in place.
    patch : DependencyPatch
        Replacement metadata describing the dependency to update.
    config : DependencyConfig
        Configuration describing how the inline dependency should be produced.
    manifest : Path
        Path to the manifest used for error reporting.

    Returns
    -------
    None
        The targeted dependency entry is updated in ``document``.

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
    >>> update_dependency(
    ...     doc,
    ...     patch,
    ...     DependencyConfig('1.0.0', include_local_path=True),
    ...     Path('Cargo.toml'),
    ... )
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
    section[patch.name] = build_inline_dependency(
        extra_items,
        patch.path,
        config.version,
        include_local_path=config.include_local_path,
    )


def extract_existing_items(value: object) -> Iterable[tuple[str, object]]:
    """Return preserved dependency metadata from an existing entry.

    Parameters
    ----------
    value : object
        Existing dependency definition, potentially a table or inline table.

    Returns
    -------
    tuple[tuple[str, object], ...]
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
    *,
    include_local_path: bool,
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
    include_local_path : bool
        When ``True`` the dependency retains the ``path`` attribute so the
        local workspace copy is used. Disable the flag for live publishing so
        manifests point at crates.io.

    Returns
    -------
    InlineTable
        Inline table ready to be inserted into the manifest document.

    Examples
    --------
    >>> inline = build_inline_dependency((), '../foo', '1.0.0', include_local_path=True)
    >>> dict(inline)
    {'path': '../foo', 'version': '1.0.0'}
    >>> dict(build_inline_dependency((), '../foo', '1.0.0', include_local_path=False))
    {'version': '1.0.0'}
    """
    dependency = inline_table()
    if include_local_path:
        dependency["path"] = path
    dependency["version"] = version
    for key, item in extra_items:
        dependency[key] = item
    dependency.trailing_comma = False
    return dependency


def main() -> None:
    """Parse CLI arguments and rewrite the requested manifest.

    Returns
    -------
    None
        Exits with status ``0`` after the manifest has been patched successfully.

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
    parser.add_argument(
        "--include-local-path",
        dest="include_local_path",
        action="store_true",
        default=True,
        help=(
            "Retain relative path dependencies for publish-checks. This is the "
            "default behaviour."
        ),
    )
    parser.add_argument(
        "--omit-local-path",
        dest="include_local_path",
        action="store_false",
        help=(
            "Drop relative path dependencies so manifests resolve crates on "
            "crates.io."
        ),
    )
    args = parser.parse_args()
    apply_replacements(
        args.crate,
        args.manifest,
        args.version,
        include_local_path=args.include_local_path,
    )


if __name__ == "__main__":
    main()
