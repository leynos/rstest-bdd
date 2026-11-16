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
    sources = {
        relative_path: read_text(root, relative_path)
        for relative_path in (PRELUDE, CRATE_ROOT, USERS_GUIDE)
    }
    violations = [
        f"{relative_path}: unreadable"
        for relative_path, source in sources.items()
        if source is None
    ]
    if violations:
        return violations

    bindings, binding_violations = prelude_bindings(sources[PRELUDE] or "")
    exports, export_violations = crate_root_exports(sources[CRATE_ROOT] or "")
    violations.extend(binding_violations)
    violations.extend(export_violations)
    for crate, item in sorted(bindings):
        if item not in exports:
            violations.append(
                f"the prelude exports {crate}::{item}, "
                f"which the crate root does not re-export"
            )

    section = find_section(sources[USERS_GUIDE] or "", SECTION_ANCHOR)
    if section is None:
        violations.append(f"{USERS_GUIDE}: no section with anchor #{SECTION_ANCHOR}")
        return violations

    violations.extend(table_violations(table_rows(section), bindings))
    violations.extend(spelled_violations(code_block_bodies(section), bindings))
    violations.extend(readme_violations(root, exports))
    return violations


def main() -> int:
    """Check the documented prelude imports and report every violation."""
    root = Path(__file__).resolve().parents[1]
    violations = check_prelude_exports(root)
    for violation in violations:
        print(violation, file=sys.stderr)
    return 1 if violations else 0


if __name__ == "__main__":
    sys.exit(main())

if __name__ == "__main__":
    sys.exit(main())
