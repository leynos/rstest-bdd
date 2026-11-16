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


def header_violations(header: list[str]) -> list[str]:
    """
    Check that the item table's header names the four expected columns.

    Parameters
    ----------
    header : list[str]
        The cells of the table's first row.

    Returns
    -------
    list[str]
        A violation when the header's columns are not ``TABLE_HEADER``.
    """
    if tuple(header) == TABLE_HEADER:
        return []
    return [
        (
            f"{USERS_GUIDE}: the import table header is {tuple(header)}; "
            f"expected {TABLE_HEADER}"
        )
    ]


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
    violations = header_violations(rows[0])
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


def first_example(bodies: list[str], marker: str) -> str | None:
    """
    Return the first fenced block that contains a marker.

    Parameters
    ----------
    bodies : list[str]
        Bodies of the section's fenced blocks, in document order.
    marker : str
        The text that identifies the example wanted.

    Returns
    -------
    str | None
        The first matching block's body, or ``None`` when no block matches.
    """
    return next((body for body in bodies if marker in body), None)


def spelled_imports(body: str) -> list[tuple[str, str]]:
    """
    Return every ``(path, name)`` pair an example's imports name.

    Parameters
    ----------
    body : str
        One fenced block's text, which may hold several ``use`` statements.

    Returns
    -------
    list[tuple[str, str]]
        One pair per imported name, in document order.
    """
    return [
        pair
        for match in IMPORT_STATEMENT.finditer(body)
        for pair in use_names(match.group("body"))
    ]


def foreign_imports(imports: list[tuple[str, str]]) -> list[str]:
    """
    Report each import that reaches past the crate root.

    Parameters
    ----------
    imports : list[tuple[str, str]]
        The ``(path, name)`` pairs the example names.

    Returns
    -------
    list[str]
        One violation per import of a path other than the crate root.
    """
    root_prefix = f"{ROOT_PREFIX}::"
    return [
        f"{USERS_GUIDE}: the spelled example imports {path}; "
        f"only the crate root should be named"
        for path, _ in imports
        if not path.startswith(root_prefix)
    ]


def rooted_names(imports: list[tuple[str, str]]) -> set[str]:
    """
    Return the names the example imports through the crate root.

    Parameters
    ----------
    imports : list[tuple[str, str]]
        The ``(path, name)`` pairs the example names.

    Returns
    -------
    set[str]
        The names reached through ``rstest_bdd`` itself.
    """
    root_prefix = f"{ROOT_PREFIX}::"
    return {name for path, name in imports if path.startswith(root_prefix)}


def name_set_violations(names: set[str], expected: set[str]) -> list[str]:
    """
    Report the names the example omits and the names it should not import.

    Parameters
    ----------
    names : set[str]
        The names the spelled example imports through the crate root.
    expected : set[str]
        The names the prelude exports.

    Returns
    -------
    list[str]
        Violations; empty when the two sets agree.
    """
    violations: list[str] = []
    for name in sorted(names ^ expected):
        if name in expected:
            violations.append(f"{USERS_GUIDE}: the spelled example omits {name}")
            continue
        violations.append(
            f"{USERS_GUIDE}: the spelled example names {name}, "
            f"which the prelude does not export"
        )
    return violations


def example_violations(example: str, bindings: set[tuple[str, str]]) -> list[str]:
    """
    Compare one spelled-out example with the prelude's bindings.

    Parameters
    ----------
    example : str
        The body of the fenced block carrying the spelled-out import.
    bindings : set[tuple[str, str]]
        The ``(defining crate, name)`` pairs the prelude exports.

    Returns
    -------
    list[str]
        Violations; empty when the example imports exactly the prelude.
    """
    imports = spelled_imports(example)
    return [
        *foreign_imports(imports),
        *name_set_violations(rooted_names(imports), {name for _, name in bindings}),
    ]


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
    if first_example(bodies, GLOB_IMPORT) is None:
        return [f"{USERS_GUIDE}: the prelude glob example is missing"]
    example = first_example(bodies, SPELLED_IMPORT)
    if example is None:
        return [f"{USERS_GUIDE}: the spelled-out import example is missing"]
    return example_violations(example, bindings)


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
