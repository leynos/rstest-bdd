"""Unit tests for the documented-prelude drift checker."""

from pathlib import Path

from check_prelude_exports import check_prelude_exports
from prelude_drift import HARNESS_CONTEXT, NO_IMPORT
from prelude_exports import MACROS_CRATE, RUNTIME_CRATE

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]

ROWS = [
    ("given", "attribute macro", MACROS_CRATE),
    ("when", "attribute macro", MACROS_CRATE),
    ("then", "attribute macro", MACROS_CRATE),
    ("ScenarioState", "derive macro", MACROS_CRATE),
    ("ScenarioState", "trait", RUNTIME_CRATE),
    ("StepError", "enum", RUNTIME_CRATE),
]

README_IMPORT = "use rstest_bdd::{given, when, then};"


def _spelled_names(rows: list[tuple[str, str, str]]) -> list[str]:
    """Return the distinct item names a table documents, in row order."""
    names: list[str] = []
    for item, _, _ in rows:
        if item not in names:
            names.append(item)
    return names


def _table(rows: list[tuple[str, str, str]]) -> str:
    """Render the guide's import table for *rows*."""
    lines = [
        "| Item | Kind | Defining crate | Canonical path |",
        "| --- | --- | --- | --- |",
    ]
    for item, kind, crate in rows:
        lines.append(f"| `{item}` | {kind} | `{crate}` | `rstest_bdd::{item}` |")
    lines.append(f"| `{HARNESS_CONTEXT}` | parameter marker | none | {NO_IMPORT} |")
    return "\n".join(lines)


def _guide(rows: list[tuple[str, str, str]], spelled: str | None = None) -> str:
    """Render a users-guide containing the prelude import section."""
    names = ", ".join(_spelled_names(rows))
    spelled_block = (
        spelled
        if spelled is not None
        else (f"```rust\nuse rstest_bdd::{{{names}}};\n```")
    )
    return (
        "## Importing `rstest-bdd`\n\n"
        "One glob supplies everything:\n\n"
        "```rust\nuse rstest_bdd::prelude::*;\n```\n\n"
        "Spelled out in full:\n\n"
        f"{spelled_block}\n\n"
        f"{_table(rows)}\n\n"
        "## Step definitions\n\ntext\n"
    )


def _prelude(rows: list[tuple[str, str, str]]) -> str:
    """Render a prelude module re-exporting *rows*."""
    macro_names = [item for item, _, crate in rows if crate == MACROS_CRATE]
    runtime_names = [item for item, _, crate in rows if crate == RUNTIME_CRATE]
    return (
        f"pub use rstest_bdd_macros::{{{', '.join(macro_names)}}};\n\n"
        f"pub use crate::{{{', '.join(runtime_names)}}};\n"
    )


def _crate_root(rows: list[tuple[str, str, str]]) -> str:
    """Render a crate root re-exporting every item *rows* names."""
    names = ", ".join(dict.fromkeys(_spelled_names(rows)))
    return f"pub mod prelude;\n\npub use modules::{{{names}}};\n"


def write_tree(root: Path, rows: list[tuple[str, str, str]], **overrides: str) -> None:
    """
    Write a repository tree whose prelude documentation agrees with *rows*.

    Parameters
    ----------
    root : Path
        The temporary repository root.
    rows : list[tuple[str, str, str]]
        The ``(item, kind, defining crate)`` rows the table and prelude share.
    **overrides : str
        Replacement file contents keyed by repository-relative path, so a test
        can desynchronize exactly one input.
    """
    files = {
        "crates/rstest-bdd/src/prelude.rs": _prelude(rows),
        "crates/rstest-bdd/src/lib.rs": _crate_root(rows),
        "docs/users-guide.md": _guide(rows),
        "README.md": f"{README_IMPORT}\n",
        "crates/rstest-bdd/README.md": "use rstest_bdd::{given};\n",
        "crates/rstest-bdd-macros/README.md": "use rstest_bdd::{when};\n",
    }
    files.update(overrides)
    for relative_path, content in files.items():
        path = root / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")


def test_synchronized_tree_passes(tmp_path: Path) -> None:
    """Return no violations when every input agrees."""
    write_tree(tmp_path, ROWS)
    violations = check_prelude_exports(tmp_path)
    assert not violations, f"unexpected violations: {violations}"


def test_missing_table_row_is_reported(tmp_path: Path) -> None:
    """Report a prelude binding that the item table does not document."""
    rows = [row for row in ROWS if row[0] != "StepError"]
    write_tree(tmp_path, ROWS, **{"docs/users-guide.md": _guide(rows)})
    violations = check_prelude_exports(tmp_path)
    assert any(
        "the table omits rstest-bdd::StepError" in item for item in violations
    ), f"expected the omission violation, got {violations}"


def test_missing_prelude_binding_is_reported(tmp_path: Path) -> None:
    """Report a documented item that the prelude no longer exports."""
    rows = [row for row in ROWS if row[0] != "then"]
    write_tree(tmp_path, rows, **{"docs/users-guide.md": _guide(ROWS)})
    violations = check_prelude_exports(tmp_path)
    assert any(
        "the table documents rstest-bdd-macros::then" in item for item in violations
    ), f"expected the stale-row violation, got {violations}"


def test_wrong_kind_is_reported(tmp_path: Path) -> None:
    """Report a row whose kind contradicts the recorded kind."""
    rows = [
        ("ScenarioState", "derive macro", RUNTIME_CRATE)
        if row == ("ScenarioState", "trait", RUNTIME_CRATE)
        else row
        for row in ROWS
    ]
    write_tree(tmp_path, rows)
    violations = check_prelude_exports(tmp_path)
    assert any(
        "documented as derive macro; expected trait" in item for item in violations
    ), f"expected the kind violation, got {violations}"


def test_spelled_example_must_not_name_the_macro_crate(tmp_path: Path) -> None:
    """Reject a spelled example that reaches the macro crate directly."""
    spelled = (
        "```rust\n"
        "use rstest_bdd_macros::{given, when, then};\n"
        "use rstest_bdd::{ScenarioState, StepError};\n"
        "```"
    )
    write_tree(tmp_path, ROWS, **{"docs/users-guide.md": _guide(ROWS, spelled=spelled)})
    violations = check_prelude_exports(tmp_path)
    assert any("only the crate root should be named" in item for item in violations), (
        f"expected the macro-crate violation, got {violations}"
    )


def test_spelled_example_must_list_every_item(tmp_path: Path) -> None:
    """Report a spelled example that omits a prelude binding."""
    spelled = "```rust\nuse rstest_bdd::{given, when, then};\n```"
    write_tree(tmp_path, ROWS, **{"docs/users-guide.md": _guide(ROWS, spelled=spelled)})
    violations = check_prelude_exports(tmp_path)
    assert any(
        "the spelled example omits ScenarioState" in item for item in violations
    ), f"expected the omission violation, got {violations}"


def test_readme_import_naming_an_unknown_export_is_reported(tmp_path: Path) -> None:
    """Report a README import that names a missing crate-root export."""
    write_tree(
        tmp_path,
        ROWS,
        **{"README.md": "use rstest_bdd::{given, SinceVanished};\n"},
    )
    violations = check_prelude_exports(tmp_path)
    assert any(
        "SinceVanished is not a crate-root export" in item for item in violations
    ), f"expected the unknown-export violation, got {violations}"


def test_missing_section_is_reported(tmp_path: Path) -> None:
    """Report a guide without the documented import section."""
    write_tree(tmp_path, ROWS, **{"docs/users-guide.md": "## Step definitions\n"})
    violations = check_prelude_exports(tmp_path)
    assert any("no section with anchor" in item for item in violations), (
        f"expected the missing-section violation, got {violations}"
    )


def test_unreadable_source_is_reported(tmp_path: Path) -> None:
    """Report a source file the check cannot read."""
    write_tree(tmp_path, ROWS)
    (tmp_path / "crates/rstest-bdd/src/prelude.rs").unlink()
    violations = check_prelude_exports(tmp_path)
    assert any("prelude.rs: unreadable" in item for item in violations), (
        f"expected the unreadable-source violation, got {violations}"
    )


def test_live_documentation_agrees_with_the_prelude() -> None:
    """Assert this repository's documentation matches its prelude."""
    violations = check_prelude_exports(REPOSITORY_ROOT)
    assert not violations, f"the documentation has drifted: {violations}"
