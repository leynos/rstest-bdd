#!/usr/bin/env python3
"""
Read what `rstest-bdd` actually exports, for the documentation drift check.

`crates/rstest-bdd/src/prelude.rs` and `crates/rstest-bdd/src/lib.rs` state the
prelude's surface in Rust. Nothing else in the repository can see that surface:
a doctest proves the names compile, but not which names there are, and prose
cannot be compared with a `pub use` unless something parses it.

This module turns both files into sets. `prelude_bindings` reports each
`(defining crate, name)` pair the prelude re-exports, and `crate_root_exports`
reports every name the crate root offers. `scripts/prelude_drift.py` compares
those sets with the documentation.

`pub use` cannot record whether a name is a derive macro, an attribute macro,
or a trait, so `ITEM_KINDS` carries that fact by hand. A prelude binding with
no recorded kind is a violation rather than an assumption, which is what makes
adding an export fail until its documentation is written.
"""

import re
from pathlib import Path

PRELUDE = Path("crates/rstest-bdd/src/prelude.rs")
CRATE_ROOT = Path("crates/rstest-bdd/src/lib.rs")
USERS_GUIDE = Path("docs/users-guide.md")
READMES = (
    Path("README.md"),
    Path("crates/rstest-bdd/README.md"),
    Path("crates/rstest-bdd-macros/README.md"),
)

SECTION_ANCHOR = "importing-rstest-bdd"

# The section opens and closes at heading levels up to this depth, so a
# subsection written inside it does not end the region.
SECTION_HEADING_LEVEL = 2
TABLE_HEADER = ("Item", "Kind", "Defining crate", "Canonical path")
NO_IMPORT = "no import required"
HARNESS_CONTEXT = "#[harness_context]"

MACROS_CRATE = "rstest-bdd-macros"
RUNTIME_CRATE = "rstest-bdd"
ROOT_PREFIX = "rstest_bdd"

GLOB_IMPORT = "use rstest_bdd::prelude::*;"
SPELLED_IMPORT = f"use {ROOT_PREFIX}::{{"

# The two defining crates a prelude item can come from, keyed by the `pub use`
# root that names them in `prelude.rs`.
DEFINING_CRATES = {"rstest_bdd_macros": MACROS_CRATE, "crate": RUNTIME_CRATE}

# The kind of each prelude item. Rust decides what an item is by namespace, not
# by spelling, so `pub use` cannot carry this: `ScenarioState` is both a derive
# macro and a trait, and the table must say which row delivers which.
ITEM_KINDS = {
    (MACROS_CRATE, "ScenarioState"): "derive macro",
    (MACROS_CRATE, "StepArgs"): "derive macro",
    (MACROS_CRATE, "given"): "attribute macro",
    (MACROS_CRATE, "scenario"): "attribute macro",
    (MACROS_CRATE, "scenarios"): "attribute macro",
    (MACROS_CRATE, "then"): "attribute macro",
    (MACROS_CRATE, "when"): "attribute macro",
    (RUNTIME_CRATE, "ScenarioState"): "trait",
    (RUNTIME_CRATE, "StepArgs"): "trait",
    (RUNTIME_CRATE, "Slot"): "struct",
    (RUNTIME_CRATE, "StepContext"): "struct",
    (RUNTIME_CRATE, "StepError"): "enum",
    (RUNTIME_CRATE, "StepResult"): "type alias",
}

USE_STATEMENT = re.compile(r"\bpub use (?P<body>[^;]*);", re.DOTALL)
IMPORT_STATEMENT = re.compile(r"\buse (?P<body>[^;]*);", re.DOTALL)
ROOT_ITEM = re.compile(
    r"^pub (?:const )?(?:fn|struct|enum|trait|type|mod) (?P<name>[A-Za-z_]\w*)",
    re.MULTILINE,
)


def split_top_level(text: str) -> list[str]:
    """
    Split a braced import list on the commas outside any nested braces.

    Parameters
    ----------
    text : str
        The text between an import's braces.

    Returns
    -------
    list[str]
        One entry per comma-separated part, with nested groups kept intact.
    """
    parts: list[str] = []
    current: list[str] = []
    depth = 0
    for char in text:
        match char:
            case "{":
                depth += 1
            case "}":
                depth -= 1
            case "," if depth == 0:
                parts.append("".join(current))
                current = []
                continue
        current.append(char)
    parts.append("".join(current))
    return [part.strip() for part in parts if part.strip()]


def use_names(tree: str, prefix: str = "") -> list[tuple[str, str]]:
    """
    Expand one import tree into ``(module path, imported name)`` pairs.

    Parameters
    ----------
    tree : str
        An import tree without its leading ``use`` or trailing ``;``.
    prefix : str
        Module path accumulated from the enclosing groups.

    Returns
    -------
    list[tuple[str, str]]
        One pair per name the tree imports.
    """
    flat = " ".join(tree.split())
    brace = flat.find("{")
    if brace == -1:
        path, alias = flat, flat.rsplit("::", 1)[-1]
        if " as " in flat:
            path, _, alias = flat.partition(" as ")
        path = path.strip()
        return [(prefix + path, alias.strip())]
    head = flat[:brace]
    inner = flat[brace + 1 : flat.rfind("}")]
    pairs: list[tuple[str, str]] = []
    for part in split_top_level(inner):
        pairs.extend(use_names(part, prefix + head))
    return pairs


def statement_bindings(body: str) -> tuple[set[tuple[str, str]], list[str]]:
    """
    Return the bindings one ``pub use`` statement contributes to the prelude.

    Parameters
    ----------
    body : str
        The text of one ``pub use`` statement, without ``pub use`` or ``;``.

    Returns
    -------
    tuple[set[tuple[str, str]], list[str]]
        The statement's bindings, and any violations found while reading them.
        A statement from an unexpected root contributes no bindings.
    """
    root, separator, rest = body.partition("::")
    if not separator or root not in DEFINING_CRATES:
        return set(), [f"prelude imports from an unexpected root: {body}"]
    defining_crate = DEFINING_CRATES[root]
    bindings: set[tuple[str, str]] = set()
    violations: list[str] = []
    for _, name in use_names(rest):
        if name == "*":
            violations.append(f"prelude glob import defeats the check: {body}")
            continue
        bindings.add((defining_crate, name))
    return bindings, violations


def prelude_bindings(source: str) -> tuple[set[tuple[str, str]], list[str]]:
    """
    Return the ``(defining crate, name)`` pairs a prelude re-exports.

    Parameters
    ----------
    source : str
        The text of ``crates/rstest-bdd/src/prelude.rs``.

    Returns
    -------
    tuple[set[tuple[str, str]], list[str]]
        The bindings, and any violations found while reading them.
    """
    bindings: set[tuple[str, str]] = set()
    violations: list[str] = []
    for match in USE_STATEMENT.finditer(source):
        statement, statement_violations = statement_bindings(
            match.group("body").strip()
        )
        bindings.update(statement)
        violations.extend(statement_violations)
    if not bindings:
        violations.append("prelude re-exports nothing; the check would be vacuous")
    return bindings, violations


def crate_root_exports(source: str) -> tuple[set[str], list[str]]:
    """
    Return the names the crate root exports, and any glob that hides them.

    Parameters
    ----------
    source : str
        The text of ``crates/rstest-bdd/src/lib.rs``.

    Returns
    -------
    tuple[set[str], list[str]]
        The exported names, and any violations found while reading them.
    """
    exports: set[str] = set()
    violations: list[str] = []
    for match in USE_STATEMENT.finditer(source):
        for _, name in use_names(match.group("body"), ""):
            if name == "*":
                violations.append("the crate root re-exports a glob wholesale")
                continue
            exports.add(name)
    exports.update(match.group("name") for match in ROOT_ITEM.finditer(source))
    if not exports:
        violations.append("the crate root exports nothing; the check would be vacuous")
    return exports, violations
