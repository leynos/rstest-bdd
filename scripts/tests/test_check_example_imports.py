"""Unit tests for the example import checker."""

from pathlib import Path

from check_example_imports import (
    MACROS_CRATE,
    check_example_imports,
    step_attribute_sources,
)

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]

FACADE_MANIFEST = """\
[package]
name = "compliant-example"
version = "0.1.0"

[dev-dependencies]
rstest-bdd.workspace = true
"""

MACROS_MANIFEST = """\
[package]
name = "violating-example"
version = "0.1.0"

[dependencies]
rstest-bdd-macros = "0.7.0"
"""

PRELUDE_SOURCE = """\
use rstest_bdd::prelude::*;

#[given("a step")]
fn a_step() {}
"""

CRATE_ROOT_SOURCE = """\
use rstest_bdd::{given};

#[given("a step")]
fn a_step() {}
"""

BRACED_PRELUDE_SOURCE = """\
use rstest_bdd::prelude::{given};

#[given("a step")]
fn a_step() {}
"""

MACROS_SOURCE = """\
use rstest_bdd_macros::given;

#[given("a step")]
fn a_step() {}
"""

MACROS_ONLY_SOURCE = """\
use rstest_bdd_macros::given;
"""

RENAMED_MACROS_MANIFEST = """\
[package]
name = "renamed-example"
version = "0.1.0"

[dev-dependencies]
macros = { package = "rstest-bdd-macros", version = "0.7.0" }
"""

TARGET_MACROS_MANIFEST = """\
[package]
name = "target-table-example"
version = "0.1.0"

[target.'cfg(unix)'.dependencies]
rstest-bdd-macros = "0.7.0"
"""

NO_FACADE_SOURCE = """\
use some_other_crate::Thing;

#[scenario(path = "tests/steps.feature")]
fn a_scenario() {}
"""

DRIVER_SOURCE = """\
use rstest_bdd_harness::binary_test_support::locate_or_build_binary;

#[test]
fn the_binary_runs() {}
"""


def write_example(
    root: Path, name: str, manifest: str, sources: dict[str, str]
) -> None:
    """
    Write one example crate beneath *root*.

    Parameters
    ----------
    root : Path
        The temporary repository root.
    name : str
        The example crate's directory name.
    manifest : str
        The manifest text to write.
    sources : dict[str, str]
        Relative source paths mapped to their contents.
    """
    crate = root / "examples" / name
    (crate / "tests").mkdir(parents=True)
    (crate / "Cargo.toml").write_text(manifest, encoding="utf-8")
    for relative, source in sources.items():
        (crate / relative).write_text(source, encoding="utf-8")


def test_passes_for_facade_only_example(tmp_path: Path) -> None:
    """A manifest and source that reach only the facade should pass."""
    write_example(
        tmp_path, "compliant", FACADE_MANIFEST, {"tests/steps.rs": PRELUDE_SOURCE}
    )

    assert not check_example_imports(tmp_path), "a compliant example should pass"


def test_reports_macros_dependency(tmp_path: Path) -> None:
    """A manifest declaring the macro crate should be named in the report."""
    write_example(
        tmp_path, "violating", MACROS_MANIFEST, {"tests/steps.rs": PRELUDE_SOURCE}
    )

    violations = check_example_imports(tmp_path)

    assert len(violations) == 1, "only the manifest should be at fault"
    assert "Cargo.toml" in violations[0], "the manifest should be identified"
    assert MACROS_CRATE in violations[0], "the offending dependency should be named"


def test_reports_macros_import(tmp_path: Path) -> None:
    """A step source importing from the macro crate should be reported."""
    write_example(
        tmp_path, "violating", FACADE_MANIFEST, {"tests/steps.rs": MACROS_SOURCE}
    )

    violations = check_example_imports(tmp_path)

    assert any("macro crate directly" in violation for violation in violations), (
        "the direct import should be reported"
    )


def test_reports_step_source_without_facade_import(tmp_path: Path) -> None:
    """A step source that reaches neither facade form should be reported."""
    write_example(
        tmp_path, "rogue", FACADE_MANIFEST, {"tests/steps.rs": NO_FACADE_SOURCE}
    )

    violations = check_example_imports(tmp_path)

    assert any("without importing" in violation for violation in violations), (
        "a missing facade import should be reported"
    )


def test_ignores_source_without_step_attributes(tmp_path: Path) -> None:
    """A driver source adds no violation beside a compliant step source."""
    write_example(
        tmp_path,
        "driver",
        FACADE_MANIFEST,
        {"tests/cli.rs": DRIVER_SOURCE, "tests/steps.rs": PRELUDE_SOURCE},
    )

    assert not check_example_imports(tmp_path), (
        "a source with no step attributes should not be checked"
    )


def test_reports_crate_root_import(tmp_path: Path) -> None:
    """A step source that imports the crate root should be reported."""
    write_example(
        tmp_path,
        "root-import",
        FACADE_MANIFEST,
        {"tests/steps.rs": CRATE_ROOT_SOURCE},
    )

    violations = check_example_imports(tmp_path)

    assert any("crate root" in violation for violation in violations), (
        "the finish line asks for the prelude, so the root import should fail"
    )


def test_accepts_a_braced_prelude_import(tmp_path: Path) -> None:
    """A braced import from the prelude is the same claim as the glob."""
    write_example(
        tmp_path,
        "braced-prelude",
        FACADE_MANIFEST,
        {"tests/steps.rs": BRACED_PRELUDE_SOURCE},
    )

    assert not check_example_imports(tmp_path), (
        "any import from the prelude satisfies the source rule"
    )


def test_reports_macro_crate_named_without_attributes(tmp_path: Path) -> None:
    """A file naming the macro crate is in scope even with no attributes."""
    write_example(
        tmp_path,
        "bare-macros-import",
        FACADE_MANIFEST,
        {"tests/support.rs": MACROS_ONLY_SOURCE},
    )

    violations = check_example_imports(tmp_path)

    assert any("macro crate directly" in violation for violation in violations), (
        "naming the macro crate alone should bring the file into scope"
    )


def test_reports_a_renamed_macros_dependency(tmp_path: Path) -> None:
    """A dependency renamed with `package` still declares the macro crate."""
    write_example(
        tmp_path,
        "renamed",
        RENAMED_MACROS_MANIFEST,
        {"tests/steps.rs": PRELUDE_SOURCE},
    )

    violations = check_example_imports(tmp_path)

    assert any(MACROS_CRATE in violation for violation in violations), (
        "the resolved package name should be reported, not the local alias"
    )


def test_reports_a_target_specific_macros_dependency(tmp_path: Path) -> None:
    """A dependency under a target-specific table is still declared."""
    write_example(
        tmp_path,
        "target-table",
        TARGET_MACROS_MANIFEST,
        {"tests/steps.rs": PRELUDE_SOURCE},
    )

    violations = check_example_imports(tmp_path)

    assert any(MACROS_CRATE in violation for violation in violations), (
        "a target-specific dependency table is not an escape hatch"
    )


def test_reports_a_scan_that_found_nothing(tmp_path: Path) -> None:
    """An empty tree must fail rather than pass vacuously."""
    violations = check_example_imports(tmp_path)

    assert any("found no example crates" in violation for violation in violations), (
        "the manifest half should report an empty scan"
    )
    assert any(
        "the source rule checked nothing" in violation for violation in violations
    ), "the source half should report an empty scan"


def test_repository_examples_use_the_prelude() -> None:
    """The live tree must pass, and the source rule must see its subjects.

    Naming the four step-bearing example test files keeps the check honest: a
    rename or a fifth example is a deliberate change to the set of files users
    copy, and the gate should say so rather than quietly scanning a different
    set.
    """
    inspected = [
        path.relative_to(REPOSITORY_ROOT).as_posix()
        for path in step_attribute_sources(REPOSITORY_ROOT)
    ]

    assert inspected == [
        "examples/gpui-counter/tests/counter.rs",
        "examples/japanese-ledger/tests/ledger.rs",
        "examples/todo-cli/tests/todo.rs",
        "examples/tokio-reminders/tests/reminders.rs",
    ], "the step-bearing example sources should be exactly these four"
    assert not check_example_imports(REPOSITORY_ROOT), (
        "the live example tree should satisfy the gate"
    )
