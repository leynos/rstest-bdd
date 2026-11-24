#!/usr/bin/env python3
"""
Check that the documented `rstest-bdd` prelude matches the crate's exports.

`rstest_bdd::prelude` is a convenience over the crate root: every name it
exports is also available at `rstest_bdd`'s root, and it adds none of its own.
That promise is worth making only while the documentation keeps it, and a
doctest cannot compare a prose table with a `pub use`. This check does:

- the prelude's bindings match the users' guide's item table, and each row's
  kind and canonical path are right;
- the guide's spelled-out import names every prelude item and nothing else,
  and reaches them through the crate root only;
- every braced import in the READMEs names a crate-root export.

It prints one line per violation to standard error and exits non-zero when it
finds any.

Usage
-----
    python3 scripts/check_prelude_exports.py

The parsing lives in `prelude_exports.py`, the document readers in
`prelude_docs.py`, and the comparisons in `prelude_drift.py`.
"""

import sys
from pathlib import Path

from prelude_docs import (
    SECTION_ANCHOR,
    USERS_GUIDE,
    code_block_bodies,
    find_section,
    read_text,
    table_rows,
)
from prelude_drift import readme_violations, spelled_violations, table_violations
from prelude_exports import CRATE_ROOT, PRELUDE, crate_root_exports, prelude_bindings


def read_sources(root: Path) -> tuple[dict[Path, str], list[str]]:
    """
    Read the three files the check compares, reporting each that is unreadable.

    Parameters
    ----------
    root : Path
        Repository root directory.

    Returns
    -------
    tuple[dict[Path, str], list[str]]
        The readable sources keyed by repository-relative path, and one
        violation per file that could not be read.
    """
    sources = {
        relative_path: read_text(root, relative_path)
        for relative_path in (PRELUDE, CRATE_ROOT, USERS_GUIDE)
    }
    unreadable = [
        f"{relative_path.as_posix()}: unreadable"
        for relative_path, source in sources.items()
        if source is None
    ]
    readable = {
        relative_path: source
        for relative_path, source in sources.items()
        if source is not None
    }
    return readable, unreadable


def rust_violations(
    prelude_source: str, crate_root_source: str
) -> tuple[set[tuple[str, str]], set[str], list[str]]:
    """
    Compare the prelude's bindings with the crate root's exports.

    Parameters
    ----------
    prelude_source : str
        The text of ``crates/rstest-bdd/src/prelude.rs``.
    crate_root_source : str
        The text of ``crates/rstest-bdd/src/lib.rs``.

    Returns
    -------
    tuple[set[tuple[str, str]], set[str], list[str]]
        The prelude's bindings, the crate root's exports, and the violations
        from reading both and from comparing them.
    """
    bindings, binding_violations = prelude_bindings(prelude_source)
    exports, export_violations = crate_root_exports(crate_root_source)
    unreachable = [
        f"the prelude exports {crate}::{item}, which the crate root does not re-export"
        for crate, item in sorted(bindings)
        if item not in exports
    ]
    return bindings, exports, [*binding_violations, *export_violations, *unreachable]


def check_prelude_exports(root: Path) -> list[str]:
    """
    Check the documented prelude imports against the crate's real exports.

    Parameters
    ----------
    root : Path
        Repository root directory.

    Returns
    -------
    list[str]
        Human-readable violations; empty when everything agrees.
    """
    sources, violations = read_sources(root)
    if violations:
        return violations

    bindings, exports, rust = rust_violations(sources[PRELUDE], sources[CRATE_ROOT])
    section = find_section(sources[USERS_GUIDE], SECTION_ANCHOR)
    if section is None:
        return [*rust, f"{USERS_GUIDE}: no section with anchor #{SECTION_ANCHOR}"]
    return [
        *rust,
        *table_violations(table_rows(section), bindings),
        *spelled_violations(code_block_bodies(section), bindings),
        *readme_violations(root, exports),
    ]


def main() -> int:
    """Check the documented prelude imports and report every violation."""
    root = Path(__file__).resolve().parents[1]
    violations = check_prelude_exports(root)
    for violation in violations:
        print(violation, file=sys.stderr)
    return 1 if violations else 0


if __name__ == "__main__":
    sys.exit(main())
