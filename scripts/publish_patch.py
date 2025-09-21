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
    dependency = inline_table()
    dependency["path"] = path
    dependency["version"] = version
    for key, item in extra_items:
        dependency[key] = item
    dependency.trailing_comma = False
    return dependency


def main() -> None:
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
