"""Regression coverage for the parser-independent Python line budget.

Managed PyPy reports nothing at all for a module it cannot parse, so the
PyPy-backed PyLint pass cannot be the only enforcement of the 400-line
budget. ``scripts/check_py_file_lengths.py`` measures every module beneath
the lint roots from its bytes instead, and these tests pin that behaviour:
an over-length module is reported whether or not a parser accepts it, and a
module the scan cannot read is reported rather than dropped from the count.

These tests run via ``make test``.
"""

import ast
import importlib
import typing as typ
from pathlib import Path

import pytest

if typ.TYPE_CHECKING:
    import types

SCRIPTS = Path(__file__).resolve().parents[1]
REPOSITORY_ROOT = SCRIPTS.parent
CHECKER = "scripts/check_py_file_lengths.py"
LINT_ROOTS = ("scripts", "tests/workflow_contracts")
# PEP 758 dropped the parentheses around multiple exception types. CPython 3.14
# accepts the form below; a parser that lags it rejects the module outright,
# which is how an over-length module slipped past the PyLint pass unnoticed.
PEP_758_TAIL = "except FileNotFoundError, IsADirectoryError:\n"


@pytest.fixture
def checker(monkeypatch: pytest.MonkeyPatch) -> types.ModuleType:
    """Import the standalone length checker from the scripts directory."""
    monkeypatch.syspath_prepend(str(SCRIPTS))
    importlib.invalidate_caches()
    return importlib.import_module("check_py_file_lengths")


def build_module(
    root: Path,
    relative: str,
    source: str,
) -> Path:
    """Write *source* to a module beneath *root* and return its relative path.

    Parameters
    ----------
    root : Path
        Temporary repository root.
    relative : str
        Repository-relative path of the module to write.
    source : str
        Full source text of the module.

    Returns
    -------
    Path
        The repository-relative path of the written module.
    """
    module = root / relative
    module.parent.mkdir(parents=True, exist_ok=True)
    module.write_text(source, encoding="utf-8")
    return Path(relative)


def filler(line_count: int, *, tail: str = "") -> str:
    """Return *line_count* lines of filler followed by *tail*."""
    return "pass\n" * line_count + tail


def recipe_of(target: str) -> list[str]:
    """Return the stripped recipe lines of one Makefile target."""
    lines = (REPOSITORY_ROOT / "Makefile").read_text(encoding="utf-8").splitlines()
    header = next(
        (index for index, line in enumerate(lines) if line.startswith(f"{target}:")),
        None,
    )
    assert header is not None, f"the Makefile must declare a {target} target"

    recipe: list[str] = []
    for line in lines[header + 1 :]:
        if not line.startswith("\t"):
            break
        recipe.append(line.strip())
    return recipe


def test_module_over_the_budget_is_reported(
    checker: types.ModuleType, tmp_path: Path
) -> None:
    """A module over the budget is a violation carrying its measured length."""
    over_budget = checker.MAX_LINES + 1
    relative = build_module(tmp_path, "scripts/over_budget.py", filler(over_budget))

    violations = checker.collect_violations(tmp_path, LINT_ROOTS, set())

    assert violations == [(relative, over_budget)], (
        f"a {over_budget}-line module must be reported, got {violations!r}"
    )


def test_module_at_the_budget_is_accepted(
    checker: types.ModuleType, tmp_path: Path
) -> None:
    """A module of exactly MAX_LINES lines is inside the budget."""
    build_module(tmp_path, "scripts/at_budget.py", filler(checker.MAX_LINES))

    violations = checker.collect_violations(tmp_path, LINT_ROOTS, set())

    assert violations == [], (
        f"the budget is a maximum, so {checker.MAX_LINES} lines must pass"
    )


def test_module_a_lagging_parser_rejects_is_still_reported(
    checker: types.ModuleType, tmp_path: Path
) -> None:
    """The count must not depend on any parser accepting the module.

    The fixture uses PEP 758 syntax, so a parser that lags CPython 3.14
    rejects the whole module and can say nothing about its length. The check
    below proves the fixture really is such a module before asserting that
    the checker still reports it.
    """
    over_budget = checker.MAX_LINES + 1
    source = filler(over_budget, tail=PEP_758_TAIL)
    measured = over_budget + 1
    with pytest.raises(SyntaxError):
        ast.parse(source, feature_version=(3, 13))
    relative = build_module(tmp_path, "scripts/lagging_parser.py", source)

    violations = checker.collect_violations(tmp_path, LINT_ROOTS, set())

    assert violations == [(relative, measured)], (
        f"an over-length module no parser accepts must still be reported, "
        f"got {violations!r}"
    )


def test_module_that_is_not_utf_8_is_still_measured(
    checker: types.ModuleType, tmp_path: Path
) -> None:
    """A module the decoder rejects is measured rather than skipped."""
    over_budget = checker.MAX_LINES + 1
    module = tmp_path / "scripts" / "not_utf_8.py"
    module.parent.mkdir(parents=True)
    module.write_bytes(b"# \xff\n" * over_budget)

    violations = checker.collect_violations(tmp_path, LINT_ROOTS, set())

    assert violations == [(Path("scripts/not_utf_8.py"), over_budget)], (
        f"an undecodable {over_budget}-line module must still be reported"
    )


def test_module_outside_the_lint_roots_is_not_scanned(
    checker: types.ModuleType, tmp_path: Path
) -> None:
    """Only the lint roots are measured, so a fixture corpus is not a target."""
    build_module(
        tmp_path, "tests/fixtures/big_fixture.py", filler(checker.MAX_LINES + 1)
    )

    violations = checker.collect_violations(tmp_path, LINT_ROOTS, set())

    assert violations == [], "modules outside the lint roots are not lint targets"


def test_allowlisted_module_is_exempt(
    checker: types.ModuleType, tmp_path: Path
) -> None:
    """An allowlisted module is exempt, and the entry's path is the key."""
    relative = build_module(
        tmp_path, "scripts/exempt.py", filler(checker.MAX_LINES + 1)
    )
    allowlist_file = tmp_path / checker.ALLOWLIST_FILE
    allowlist_file.parent.mkdir(parents=True, exist_ok=True)
    allowlist_file.write_text(
        f"# tracked for refactor: see issue #0\n{relative.as_posix()}\n",
        encoding="utf-8",
    )

    allowlist = checker.load_allowlist(tmp_path)

    assert allowlist == {relative}, "the allowlist must carry the declared path"
    assert checker.collect_violations(tmp_path, LINT_ROOTS, allowlist) == [], (
        "an allowlisted module must not be reported"
    )


def test_allowlist_entry_without_a_module_is_reported(
    checker: types.ModuleType, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """An allowlist entry for a module that no longer exists is an error."""
    allowlist = {Path("scripts/removed.py")}

    assert checker.check_allowlist_integrity(tmp_path, allowlist) == 1, (
        "an allowlist entry with no module behind it must fail the check"
    )
    assert "scripts/removed.py" in capsys.readouterr().err, (
        "the missing allowlisted module must be named in the report"
    )


def test_absent_lint_root_is_reported(
    checker: types.ModuleType, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """A lint root that is not there is reported, never scanned as empty."""
    (tmp_path / "scripts").mkdir()

    assert checker.check_lint_roots(tmp_path, LINT_ROOTS) == 1, (
        "a lint root that is not there must fail the check"
    )
    assert "tests/workflow_contracts" in capsys.readouterr().err, (
        "the absent lint root must be named in the report"
    )


def test_make_lint_measures_each_pylint_target() -> None:
    """The budget runs over the same roots as the PyPy-backed PyLint pass."""
    assert f"$(PROJECT_PYTHON) {CHECKER} $(PYLINT_TARGETS)" in recipe_of(
        "lint-python"
    ), "make lint must run the line budget over every PyLint target"


def test_live_lint_roots_are_within_budget(checker: types.ModuleType) -> None:
    """The shipped tree passes the very check ``make lint`` runs."""
    assert (REPOSITORY_ROOT / checker.ALLOWLIST_FILE).is_file(), (
        "the allowlist file documents the exemption convention and must exist"
    )

    assert checker.main(LINT_ROOTS) == 0, "the live tree must be inside the budget"
