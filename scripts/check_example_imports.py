#!/usr/bin/env python3
"""
Check that the workspace examples reach `rstest-bdd` through its prelude.

The examples are the code users copy from, so an import that the facade makes
unnecessary teaches the wrong shape. Roadmap 11.2.2's finish line names the
prelude rather than the crate root, and these claims enforce it:

* No example manifest declares ``rstest-bdd-macros``, under that name or as a
  renamed ``package`` in any dependency table, including a target-specific
  one. This half is unconditional, because declaring the crate is what lets a
  downstream crate reach past the facade.
* Every example source that writes a scenario or step attribute imports the
  prelude, does not name the crate root, and names no macro crate. Reach is
  matched by path rather than by statement, so a qualified attribute such as
  ``#[rstest_bdd::given(...)]`` and a raw ``rstest_bdd_macros::scenarios!(...)``
  call are caught alongside the ``use`` forms they replace. This half is
  conditional: most example sources, including the CLI driver, never write
  those attributes and are therefore out of scope.

Both halves report when they find no subject at all. A gate whose search
silently matches nothing would otherwise pass for the wrong reason.

Usage
-----
python3 scripts/check_example_imports.py

Exit codes
----------
0
    Every example reaches `rstest-bdd` through the prelude.
1
    A manifest or source violates either claim, a file could not be read, or
    the scan found nothing to check.
"""

import re
import sys
import tomllib
import typing as typ
from pathlib import Path

EXAMPLES = Path("examples")
FACADE_CRATE = "rstest-bdd"
ROOT_PREFIX = "rstest_bdd"
MACROS_CRATE = "rstest-bdd-macros"
DEPENDENCY_TABLES = ("dependencies", "dev-dependencies", "build-dependencies")
# A scenario or step attribute, written bare or with any path prefix, so
# `#[given(...)]` and `#[rstest_bdd::given(...)]` both select the file.
STEP_ATTRIBUTE_PATH = re.compile(
    r"#\[(?:[A-Za-z_][A-Za-z0-9_]*::)*(?:scenarios|scenario|given|when|then)\b"
)
PRELUDE_IMPORT_MARKER = f"use {ROOT_PREFIX}::prelude"

# A path into the facade that is not the prelude. Matching the path rather than
# the `use` statement is what catches a qualified attribute such as
# `#[rstest_bdd::given(...)]`, which reaches the same item without importing it.
CRATE_ROOT_PATH = re.compile(rf"\b{ROOT_PREFIX}::(?!prelude\b)")
# Any mention of the macro crate: an import, a qualified attribute, or a raw
# path into a function-like macro such as `rstest_bdd_macros::scenarios!(...)`.
MACROS_CRATE_PATH = re.compile(rf"\b{ROOT_PREFIX}_macros\b")

# Each way a source can reach past the prelude, with the remedy to report.
REACH_RULES = (
    (
        MACROS_CRATE_PATH,
        "names the macro crate directly",
        f"import the attributes from the {FACADE_CRATE} facade instead",
    ),
    (
        CRATE_ROOT_PATH,
        f"reaches the {FACADE_CRATE} crate root",
        f"an example reaches the facade through `use {ROOT_PREFIX}::prelude::*;` alone",
    ),
)

if typ.TYPE_CHECKING:
    import collections.abc as cabc


def example_directories(root: Path) -> list[Path]:
    """
    Return the example crate directories beneath *root*, sorted by name.

    Parameters
    ----------
    root : Path
        The repository root.

    Returns
    -------
    list[Path]
        Each directory directly beneath ``examples``; empty when the
        directory does not exist.
    """
    examples = root / EXAMPLES
    if not examples.is_dir():
        return []
    return sorted(path for path in examples.iterdir() if path.is_dir())


def source_files(root: Path) -> list[Path]:
    """
    Return every Rust source of every example crate, sorted by path.

    Parameters
    ----------
    root : Path
        The repository root.

    Returns
    -------
    list[Path]
        Every ``.rs`` file beneath an example crate directory.
    """
    return sorted(
        path for example in example_directories(root) for path in example.rglob("*.rs")
    )


def dependency_names(specs: dict[str, object]) -> cabc.Iterator[str]:
    """
    Yield every crate one dependency table declares, by every spelling.

    Parameters
    ----------
    specs : dict[str, object]
        The contents of one dependency table, mapping each dependency's key to
        its specification.

    Yields
    ------
    str
        Each dependency's key, and its ``package`` rename when it has one. A
        key is the name the example's source writes, which is not the crate it
        resolves to once the entry is renamed, so
        ``macros = { package = "rstest-bdd-macros" }`` is found under either
        spelling.
    """
    for name, spec in specs.items():
        yield name
        package = spec.get("package") if isinstance(spec, dict) else None
        if isinstance(package, str):
            yield package


def dependency_crates(table: dict[str, object]) -> cabc.Iterator[str]:
    """
    Yield every crate a parsed manifest subtree depends on.

    The walk recurses, so a dependency declared in a target-specific table such
    as ``[target.'cfg(unix)'.dependencies]`` is found as well as one in a
    top-level table.

    Parameters
    ----------
    table : dict[str, object]
        A parsed TOML table, which may hold nested tables.

    Yields
    ------
    str
        Each dependency's table key, and its ``package`` rename when it has
        one.
    """
    for key, value in table.items():
        if not isinstance(value, dict):
            continue
        if key in DEPENDENCY_TABLES:
            yield from dependency_names(value)
        else:
            yield from dependency_crates(value)


def declared_dependencies(manifest: Path) -> set[str]:
    """
    Return every crate *manifest* depends on, by every spelling it uses.

    Parameters
    ----------
    manifest : Path
        A crate manifest to parse.

    Returns
    -------
    set[str]
        Dependency keys and their ``package`` renames, from the normal,
        development and build tables and from any target-specific table. A
        manifest that is not valid TOML propagates ``tomllib``'s decode error
        to the caller, which reports it against the file.
    """
    data = tomllib.loads(manifest.read_text(encoding="utf-8"))
    return set(dependency_crates(data))


def is_in_scope(source: str) -> bool:
    """
    Return whether *source* is subject to the source rule.

    A file qualifies by writing an attribute the facade provides, or by naming
    the macro crate at all. The second half matters: without it, a file whose
    only reach past the facade is a macro-crate import would escape the rule
    that exists to catch exactly that.

    Parameters
    ----------
    source : str
        The text of one Rust source file.

    Returns
    -------
    bool
        ``True`` when the source rule applies to the file.
    """
    return STEP_ATTRIBUTE_PATH.search(source) is not None or (
        MACROS_CRATE_PATH.search(source) is not None
    )


def step_attribute_sources(root: Path) -> list[Path]:
    """
    Return the example sources the source rule applies to.

    Parameters
    ----------
    root : Path
        The repository root.

    Returns
    -------
    list[Path]
        The in-scope sources, sorted by path.
    """
    return [
        path
        for path in source_files(root)
        if is_in_scope(path.read_text(encoding="utf-8"))
    ]


def manifest_violations(root: Path) -> list[str]:
    """
    Report every example manifest that declares the macro crate.

    Parameters
    ----------
    root : Path
        The repository root.

    Returns
    -------
    list[str]
        Human-readable violations; empty when every example is compliant.
    """
    examples = example_directories(root)
    if not examples:
        return [f"found no example crates beneath {root / EXAMPLES}"]

    violations: list[str] = []
    for example in examples:
        manifest = example / "Cargo.toml"
        if not manifest.is_file():
            violations.append(f"{manifest}: example crate has no manifest")
            continue
        try:
            declared = declared_dependencies(manifest)
        except (OSError, tomllib.TOMLDecodeError) as error:
            violations.append(f"{manifest}: could not be read: {error}")
            continue
        if MACROS_CRATE in declared:
            violations.append(
                f"{manifest}: names {MACROS_CRATE}; the {FACADE_CRATE} facade "
                "re-exports it, so no example should depend on it"
            )
    return violations


def _reach_violations(path: Path, source: str) -> list[str]:
    """
    Report every way *source* reaches the facade outside the prelude.

    Parameters
    ----------
    path : Path
        The source file the violations belong to.
    source : str
        The text of that file.

    Returns
    -------
    list[str]
        One violation per distinct form the source uses; empty when it stays
        inside the prelude.
    """
    return [
        f"{path}: {description} ({match.group()!r}); {remedy}"
        for pattern, description, remedy in REACH_RULES
        if (match := pattern.search(source))
    ]


def _step_source_violations(path: Path, source: str) -> list[str]:
    """
    Return the facade violations for one step-bearing source.

    Parameters
    ----------
    path : Path
        The source file the violations belong to.
    source : str
        The text of that file.

    Returns
    -------
    list[str]
        Human-readable violations; empty when the source is compliant.
    """
    violations = _reach_violations(path, source)
    if PRELUDE_IMPORT_MARKER not in source:
        violations.append(
            f"{path}: writes scenario or step attributes without importing the "
            f"{FACADE_CRATE} prelude; use `use {ROOT_PREFIX}::prelude::*;`"
        )
    return violations


def source_violations(root: Path) -> list[str]:
    """
    Report every step-bearing example source that reaches past the facade.

    Parameters
    ----------
    root : Path
        The repository root.

    Returns
    -------
    list[str]
        Human-readable violations; empty when every in-scope source is
        compliant.
    """
    violations: list[str] = []
    inspected = 0
    for path in source_files(root):
        try:
            source = path.read_text(encoding="utf-8")
        except OSError as error:
            violations.append(f"{path}: could not be read: {error}")
            continue
        if not is_in_scope(source):
            continue
        inspected += 1
        violations.extend(_step_source_violations(path, source))
    if inspected == 0:
        violations.append(
            "found no example source writing a scenario or step attribute; "
            "the source rule checked nothing"
        )
    return violations


def check_example_imports(root: Path) -> list[str]:
    """
    Return every violation of the examples' facade-only import policy.

    Parameters
    ----------
    root : Path
        The repository root.

    Returns
    -------
    list[str]
        Manifest violations followed by source violations.
    """
    return manifest_violations(root) + source_violations(root)


def main() -> int:
    """Check the example crates and report violations."""
    root = Path(__file__).resolve().parents[1]
    violations = check_example_imports(root)
    for violation in violations:
        print(violation, file=sys.stderr)
    return 1 if violations else 0


if __name__ == "__main__":
    sys.exit(main())
