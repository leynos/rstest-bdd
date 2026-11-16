#!/usr/bin/env python3
"""
Check that the workspace examples reach `rstest-bdd` through its public facade.

The examples are the code users copy from, so an import that the facade makes
unnecessary teaches the wrong shape. Two claims are checked:

* No example manifest declares ``rstest-bdd-macros``. This is unconditional,
  because declaring it is what lets a downstream crate reach past the facade.
* Every example source that writes a scenario or step attribute imports the
  facade and names no macro crate. This half is conditional: most example
  sources, including the CLI driver, never write those attributes and are
  therefore out of scope.

Both halves report when they find no subject at all. A gate whose search
silently matches nothing would otherwise pass for the wrong reason.

Usage
-----
python3 scripts/check_example_imports.py

Exit codes
----------
0
    Every example reaches `rstest-bdd` through the facade.
1
    A manifest or source violates either claim, a file could not be read, or
    the scan found nothing to check.
"""

import sys
import tomllib
from pathlib import Path

EXAMPLES = Path("examples")
FACADE_CRATE = "rstest-bdd"
MACROS_CRATE = "rstest-bdd-macros"
DEPENDENCY_TABLES = ("dependencies", "dev-dependencies", "build-dependencies")
STEP_ATTRIBUTE_MARKERS = ("#[scenario", "#[given", "#[when", "#[then")
FACADE_IMPORT_MARKERS = ("use rstest_bdd::prelude::*;", "use rstest_bdd::{")
MACROS_IMPORT_MARKERS = ("use rstest_bdd_macros", "#[rstest_bdd_macros::")


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


def writes_step_attributes(source: str) -> bool:
    """
    Return whether *source* writes a scenario or step attribute.

    Parameters
    ----------
    source : str
        The text of one Rust source file.

    Returns
    -------
    bool
        ``True`` when the source uses any attribute the facade provides.
    """
    return any(marker in source for marker in STEP_ATTRIBUTE_MARKERS)


def declared_dependencies(manifest: Path) -> set[str]:
    """
    Return every dependency name declared by *manifest*.

    Parameters
    ----------
    manifest : Path
        A crate manifest to parse.

    Returns
    -------
    set[str]
        Dependency names from the normal, development and build tables. A
        manifest that is not valid TOML propagates ``tomllib``'s decode error
        to the caller, which reports it against the file.
    """
    data = tomllib.loads(manifest.read_text(encoding="utf-8"))
    names: set[str] = set()
    for table in DEPENDENCY_TABLES:
        section = data.get(table, {})
        if isinstance(section, dict):
            names.update(section)
    return names


def step_attribute_sources(root: Path) -> list[Path]:
    """
    Return the example sources that write a scenario or step attribute.

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
        if writes_step_attributes(path.read_text(encoding="utf-8"))
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
    violations = [
        f"{path}: names the macro crate directly ({marker!r}); import the "
        f"attributes from the {FACADE_CRATE} facade instead"
        for marker in MACROS_IMPORT_MARKERS
        if marker in source
    ]
    if not any(marker in source for marker in FACADE_IMPORT_MARKERS):
        violations.append(
            f"{path}: writes scenario or step attributes without importing the "
            f"{FACADE_CRATE} facade; use `use rstest_bdd::prelude::*;`"
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
        if not writes_step_attributes(source):
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
