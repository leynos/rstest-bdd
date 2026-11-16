#!/usr/bin/env python3
"""
Compare the prelude's exports with what the documentation says about them.

Three inputs must agree: the prelude module, the users' guide's import table
and spelled-out example, and the braced imports in the READMEs. Each check here
reports a violation rather than raising, and each also reports when it found
nothing to check, so an empty table, a missing example, or an unchecked README
cannot pass silently. That matters more than any single rule: a drift check
whose inputs quietly disappear is worse than none, because it still reports
success.

`scripts/check_prelude_exports.py` assembles these violations into one exit
status.
"""

import typing as typ

from prelude_docs import READMES, USERS_GUIDE, read_text
from prelude_exports import IMPORT_STATEMENT, ITEM_KINDS, ROOT_PREFIX, use_names

if typ.TYPE_CHECKING:
    from pathlib import Path

TABLE_HEADER = ("Item", "Kind", "Defining crate", "Canonical path")
NO_IMPORT = "no import required"
HARNESS_CONTEXT = "#[harness_context]"

GLOB_IMPORT = f"use {ROOT_PREFIX}::prelude::*;"
SPELLED_IMPORT = f"use {ROOT_PREFIX}::{{"


def binding_row_violations(
    item: str, kind: str, defining_crate: str, canonical_path: str
) -> list[str]:
    """
    Check one binding row's kind and canonical path.

    Parameters
    ----------
    item : str
        The imported name the row documents.
    kind : str
        The item kind the row claims.
    defining_crate : str
        The crate the row says defines the item.
    canonical_path : str
        The path the row says a user writes.

    Returns
    -------
    list[str]
        Violations; empty when the row agrees with the prelude and the root.
    """
    violations: list[str] = []
    expected_path = f"{ROOT_PREFIX}::{item}"
    if canonical_path != expected_path:
        violations.append(
            f"{USERS_GUIDE}: {item} has canonical path {canonical_path}; "
            f"expected {expected_path}"
        )
    expected_kind = ITEM_KINDS.get((defining_crate, item))
    if expected_kind is None:
        violations.append(
            f"{USERS_GUIDE}: no kind is recorded for "
            f"{defining_crate}::{item}; record it in ITEM_KINDS"
        )
    elif kind != expected_kind:
        violations.append(
            f"{USERS_GUIDE}: {item} is documented as {kind}; expected {expected_kind}"
        )
    return violations


def documented_row(row: list[str]) -> tuple[list[str], tuple[str, str] | None]:
    """
    Return one table row's violations and the binding it documents.

    Parameters
    ----------
    row : list[str]
        The four cells of one data row.

    Returns
    -------
    tuple[list[str], tuple[str, str] | None]
        The row's violations, and its ``(defining crate, item)`` binding when
        the row documents a prelude export rather than a marker.
    """
    if len(row) != len(TABLE_HEADER):
        return [f"{USERS_GUIDE}: malformed import row {row}"], None
    item, kind, defining_crate, canonical_path = row
    if canonical_path == NO_IMPORT:
        if item != HARNESS_CONTEXT:
            return [
                (
                    f"{USERS_GUIDE}: {item} claims to need no import; only "
                    f"{HARNESS_CONTEXT} does"
                )
            ], None
        return [], None
    violations = binding_row_violations(item, kind, defining_crate, canonical_path)
    return violations, (defining_crate, item)


def table_violations(
    rows: list[list[str]], bindings: set[tuple[str, str]]
) -> list[str]:
    """
    Compare the documented item table with the prelude's bindings.

    Parameters
    ----------
    rows : list[list[str]]
        Cells of the guide's import table.
    bindings : set[tuple[str, str]]
        The ``(defining crate, name)`` pairs the prelude exports.

    Returns
    -------
    list[str]
        Violations; empty when every row and binding agrees.
    """
    if not rows:
        return [f"{USERS_GUIDE}: the import table is missing"]
    violations: list[str] = []
    if tuple(rows[0]) != TABLE_HEADER:
        violations.append(
            f"{USERS_GUIDE}: the import table header is {tuple(rows[0])}; "
            f"expected {TABLE_HEADER}"
        )
    documented: set[tuple[str, str]] = set()
    for row in rows[2:]:
        row_violations, binding = documented_row(row)
        violations.extend(row_violations)
        if binding is not None:
            documented.add(binding)
    for crate, item in sorted(bindings - documented):
        violations.append(f"{USERS_GUIDE}: the table omits {crate}::{item}")
    for crate, item in sorted(documented - bindings):
        violations.append(
            f"{USERS_GUIDE}: the table documents {crate}::{item}, "
            f"which the prelude does not export"
        )
    return violations


def spelled_violations(bodies: list[str], bindings: set[tuple[str, str]]) -> list[str]:
    """
    Compare the guide's spelled-out example with the prelude's bindings.

    Parameters
    ----------
    bodies : list[str]
        Bodies of the section's fenced blocks.
    bindings : set[tuple[str, str]]
        The ``(defining crate, name)`` pairs the prelude exports.

    Returns
    -------
    list[str]
        Violations; empty when the example is present and agrees.
    """
    if not any(GLOB_IMPORT in body for body in bodies):
        return [f"{USERS_GUIDE}: the prelude glob example is missing"]
    spelled = [body for body in bodies if SPELLED_IMPORT in body]
    if not spelled:
        return [f"{USERS_GUIDE}: the spelled-out import example is missing"]
    names: set[str] = set()
    violations: list[str] = []
    for match in IMPORT_STATEMENT.finditer(spelled[0]):
        for path, name in use_names(match.group("body")):
            if not path.startswith(f"{ROOT_PREFIX}::"):
                violations.append(
                    f"{USERS_GUIDE}: the spelled example imports {path}; "
                    f"only the crate root should be named"
                )
                continue
            names.add(name)
    expected = {name for _, name in bindings}
    violations.extend(
        f"{USERS_GUIDE}: the spelled example omits {name}"
        for name in sorted(expected - names)
    )
    violations.extend(
        f"{USERS_GUIDE}: the spelled example names {name}, "
        f"which the prelude does not export"
        for name in sorted(names - expected)
    )
    return violations


def import_line_violations(
    relative_path: Path, number: int, line: str, exports: set[str]
) -> tuple[bool, list[str]]:
    """
    Check one documentation line for braced crate-root imports.

    Parameters
    ----------
    relative_path : Path
        The document the line came from, for the message.
    number : int
        The line's one-based number, for the message.
    line : str
        The line to inspect.
    exports : set[str]
        Names the crate root exports.

    Returns
    -------
    tuple[bool, list[str]]
        Whether the line carried a braced import, and its violations.
    """
    start = line.find(SPELLED_IMPORT)
    if start == -1:
        return False, []
    end = line.find("}", start)
    if end == -1:
        return False, [
            f"{relative_path}:{number}: the import is not closed on its line"
        ]
    names = [name for _, name in use_names(line[start + len("use ") : end + 1])]
    return True, [
        f"{relative_path}:{number}: {name} is not a crate-root export"
        for name in names
        if name not in exports
    ]


def readme_violations(root: Path, exports: set[str]) -> list[str]:
    """
    Check that every braced README import names a crate-root export.

    Parameters
    ----------
    root : Path
        Repository root directory.
    exports : set[str]
        Names the crate root exports.

    Returns
    -------
    list[str]
        Violations; empty when every checked import resolves.
    """
    violations: list[str] = []
    checked = 0
    for relative_path in READMES:
        source = read_text(root, relative_path)
        if source is None:
            violations.append(f"{relative_path}: unreadable")
            continue
        for number, line in enumerate(source.splitlines(), start=1):
            counted, line_violations = import_line_violations(
                relative_path, number, line, exports
            )
            checked += int(counted)
            violations.extend(line_violations)
    if checked == 0:
        violations.append("no braced README import was checked; the rule is vacuous")
    return violations
