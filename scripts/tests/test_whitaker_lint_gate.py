"""Integration coverage for the Whitaker lint gate."""

import itertools
import os
import shutil
import subprocess  # ruff: ignore[suspicious-subprocess-import] - integration test invokes trusted local tooling.
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
WHITAKER_LINT = "no_unwrap_or_else_panic"
LINT_WHITAKER_RULE = "lint-whitaker:"
WHITAKER_TEST_TARGET = REPO_ROOT / "target" / "pytest-whitaker-fixtures"


def whitaker_tooling_available() -> bool:
    """Return whether the whitaker wrapper and cargo-dylint are installed."""
    return (
        shutil.which("whitaker") is not None
        and shutil.which("cargo-dylint") is not None
    )


pytestmark = pytest.mark.skipif(
    not whitaker_tooling_available(),
    reason="whitaker and cargo-dylint are only installed in tooling environments",
)


def make_executable() -> str:
    """Return the absolute Make executable used by integration tests."""
    executable = shutil.which("make")
    assert executable is not None, "make executable should be available"
    return executable


def write_lint_fixture(crate_dir: Path, lib_rs: str) -> Path:
    """Create a minimal crate for exercising the Dylint invocation."""
    (crate_dir / "src").mkdir(parents=True)
    (crate_dir / "Cargo.toml").write_text(
        "\n".join([
            "[package]",
            'name = "whitaker-lint-fixture"',
            'version = "0.0.0"',
            'edition = "2024"',
            "",
            "[lib]",
            'path = "src/lib.rs"',
            "",
        ]),
        encoding="utf-8",
    )
    (crate_dir / "src" / "lib.rs").write_text(lib_rs, encoding="utf-8")
    return crate_dir / "Cargo.toml"


def run_lint_whitaker(manifest_path: Path) -> subprocess.CompletedProcess[str]:
    """Run the Makefile target against a fixture crate."""
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(WHITAKER_TEST_TARGET)
    # The Makefile default must be exercised, so drop any stray WHITAKER
    # environment value injected by the surrounding shell.
    env.pop("WHITAKER", None)
    make = make_executable()
    # The executable and arguments are controlled by this test and the repo.
    return subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - test controls executable and arguments.
        [
            make,
            "--no-print-directory",
            "lint-whitaker",
            f"CARGO_FLAGS=--manifest-path {manifest_path} --all-targets",
        ],
        cwd=REPO_ROOT,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=False,
        timeout=1200,
    )


def extract_lint_whitaker_recipe(makefile: str) -> str:
    """Return the recipe lines `lint-whitaker` runs, and nothing after them.

    A Make recipe ends at the first line that is not tab-indented, so the scan
    must stop there. Searching the whole remainder for the first tab-indented
    line instead would run past the end of an emptied `lint-whitaker` into
    whatever target came next. A later target that invokes the suite would
    then satisfy both of the caller's assertions while the gate ran nothing,
    which is precisely the failure they exist to catch.

    Parameters
    ----------
    makefile : str
        The full text of the Makefile to read the rule from.

    Returns
    -------
    str
        The rule's tab-indented lines joined by newlines, with the tabs kept,
        since the assertions search them for the command-line flags. A Makefile
        with no `lint-whitaker` rule, or one whose rule has no recipe, fails the
        test that called this helper rather than returning an empty string:
        both mean the gate has nothing to run, and an empty recipe must not be
        handed on to assertions that would then have nothing to check.
    """
    lines = makefile.splitlines()
    targets = [
        index for index, line in enumerate(lines) if line.startswith(LINT_WHITAKER_RULE)
    ]
    assert targets, "Makefile declares a lint-whitaker target"
    recipe = list(itertools.takewhile(_is_recipe_line, lines[targets[0] + 1 :]))
    assert recipe, "lint-whitaker target has a recipe"
    return "\n".join(recipe)


def _is_recipe_line(line: str) -> bool:
    """Return whether a Makefile line belongs to the preceding rule's recipe.

    Parameters
    ----------
    line : str
        One line of the Makefile, with any trailing newline already removed.

    Returns
    -------
    bool
        ``True`` for a line that is tab-indented, and ``False`` for a blank
        line, a comment, a variable assignment, or the header of the next
        rule. That last case is what ends the scan.
    """
    return line.startswith("\t")


def read_lint_whitaker_recipe() -> str:
    """Return the recipe lines the repository's `lint-whitaker` target runs."""
    makefile = (REPO_ROOT / "Makefile").read_text(encoding="utf-8")
    return extract_lint_whitaker_recipe(makefile)


def test_lint_whitaker_target_accepts_clean_fixture(tmp_path: Path) -> None:
    """A clean fixture proves the Makefile target invokes the Whitaker suite."""
    manifest_path = write_lint_fixture(
        tmp_path,
        "\n".join([
            "//! Clean Whitaker fixture crate.",
            "pub fn clean_value(value: Option<u32>) -> u32 {",
            "    let Some(number) = value else {",
            '        panic!("missing value");',
            "    };",
            "    number",
            "}",
            "",
        ]),
    )

    result = run_lint_whitaker(manifest_path)

    assert result.returncode == 0, result.stdout


def test_lint_whitaker_selects_the_suite_by_name() -> None:
    """The gate must not use `--all`, which loads whatever the tree holds.

    `--all` loads every library present in the installed tree. That tree is
    written by a pinned installer against a rolling release, so an installer
    older than a lint cannot prune that lint's standalone library; both it and
    the suite then register the same lint name and rustc panics with
    "duplicate specification of lint" before this repository is checked. Naming
    the aggregated suite keeps the lint set fixed by this repository instead of
    by installer bookkeeping.
    """
    recipe = read_lint_whitaker_recipe()

    assert "--lib whitaker_suite" in recipe, recipe
    assert "--all" not in recipe, recipe


def test_recipe_extraction_stops_at_the_next_target() -> None:
    """An emptied rule must not borrow a later target's suite invocation.

    Without the stop at the next target, this Makefile would yield the
    fixture's `--lib whitaker_suite` line, and both assertions in the suite
    test above would pass against a `lint-whitaker` that runs no recipe at all.
    The assertion is on the refusal, because a scan that returned the borrowed
    line raises nothing and the defect would go unnoticed.
    """
    makefile = "\n".join([
        "lint-whitaker: ## A rule someone emptied by mistake",
        "",
        "lint-python: ## A later target that does invoke the suite",
        "\t$(WHITAKER) --lib whitaker_suite -- $(CARGO_FLAGS)",
        "",
    ])

    with pytest.raises(AssertionError, match="lint-whitaker target has a recipe"):
        extract_lint_whitaker_recipe(makefile)


def test_lint_whitaker_target_rejects_panicking_unwrap_or_else(
    tmp_path: Path,
) -> None:
    """A bad fixture proves the gate detects the banned panic shape."""
    manifest_path = write_lint_fixture(
        tmp_path,
        "\n".join([
            "//! Whitaker fixture crate with a banned panic shape.",
            "pub fn rejected_value(value: Option<u32>) -> u32 {",
            '    value.unwrap_or_else(|| panic!("missing value"))',
            "}",
            "",
        ]),
    )

    result = run_lint_whitaker(manifest_path)

    assert result.returncode != 0, result.stdout
    assert WHITAKER_LINT in result.stdout, result.stdout
    assert "unwrap_or_else" in result.stdout, result.stdout
