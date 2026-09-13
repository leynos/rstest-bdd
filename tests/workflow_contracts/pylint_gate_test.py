"""Verify the Pylint gate enforces the budget and reports what it cannot read.

The baseline contract asserts the configured values, which is not the same as
running the pass: a pool pinned to a single worker, or a command that keeps
the checked text while bypassing the configured messages, would satisfy every
string assertion. These contracts run the repository's own Pylint command over
modules written to break the budget and to defeat the parser, and evaluate the
worker-pool rule at widths the machine running the suite cannot supply.

Run via ``make test-workflow-contracts``.
"""

import shutil
import subprocess  # ruff: ignore[suspicious-subprocess-import] - runs the trusted local Makefile.
from pathlib import Path

import pytest

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
MODULE_LINE_BUDGET = 400
# `--eval` adds a target the Makefile does not define, and a variable set on
# the command line overrides the `?=` default it feeds.
PROBE_TARGET = "pylint-probe"
SHOW_TARGET = "show-pylint-jobs"


def _make_executable() -> str:
    """Return the absolute Make executable used by these contracts.

    Returns
    -------
    str
        The path to Make.
    """
    executable = shutil.which("make")
    assert executable is not None, "make must be available to run the gate"
    return executable


def _run_make(*arguments: str) -> subprocess.CompletedProcess[str]:
    """Run the repository Makefile and return the completed process.

    Parameters
    ----------
    *arguments : str
        Arguments placed after the Make executable.

    Returns
    -------
    subprocess.CompletedProcess[str]
        The result, with both streams captured as text.
    """
    return subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - arguments built by this test.
        [_make_executable(), "--no-print-directory", *arguments],
        cwd=REPOSITORY_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )


def _run_configured_pylint(*modules: Path) -> subprocess.CompletedProcess[str]:
    """Run the repository's Pylint command over probe modules.

    The recipe expands ``$(PYLINT)`` from the Makefile, so the interpreter,
    the worker pool, and the configuration the pass reads are the ones the
    lint gate uses.

    Parameters
    ----------
    *modules : Path
        The probe modules to lint.

    Returns
    -------
    subprocess.CompletedProcess[str]
        The result, with both streams captured as text.
    """
    recipe = f"{PROBE_TARGET}: ; @$(PYLINT) $(PYLINT_PROBE_MODULES)"
    return _run_make(
        "--eval",
        recipe,
        PROBE_TARGET,
        f"PYLINT_PROBE_MODULES={' '.join(str(module) for module in modules)}",
    )


def _module_of_lines(tmp_path: Path, lines: int) -> Path:
    """Write a module of exactly ``lines`` assignments and return its path.

    Parameters
    ----------
    tmp_path : Path
        The directory to write the module into.
    lines : int
        The number of lines the module must have.

    Returns
    -------
    Path
        The written module.
    """
    module = tmp_path / "over_budget.py"
    module.write_text(
        "".join(f"VALUE_{index} = {index}\n" for index in range(lines)),
        encoding="utf-8",
    )
    return module


def _jobs_for(cpus: int) -> int:
    """Return the worker pool the Makefile derives from ``cpus`` CPUs.

    Parameters
    ----------
    cpus : int
        The CPU count to feed the rule.

    Returns
    -------
    int
        The pool width the Makefile prints.
    """
    recipe = f"{SHOW_TARGET}: ; @echo $(PYLINT_JOBS)"
    result = _run_make("--eval", recipe, SHOW_TARGET, f"PYLINT_CPUS={cpus}")
    assert result.returncode == 0, result.stderr
    return int(result.stdout.strip())


def test_an_over_budget_module_fails_the_configured_pass(tmp_path: Path) -> None:
    """The pass must report the budget the configuration sets."""
    module = _module_of_lines(tmp_path, MODULE_LINE_BUDGET + 1)
    over_budget = f"({MODULE_LINE_BUDGET + 1}/{MODULE_LINE_BUDGET})"

    result = _run_configured_pylint(module)

    assert result.returncode != 0, result.stdout
    assert "C0302" in result.stdout, result.stdout
    assert over_budget in result.stdout, result.stdout


def test_a_module_no_parser_accepts_is_reported(tmp_path: Path) -> None:
    """A module the pass cannot read must fail, not pass in silence."""
    module = tmp_path / "syntax_error.py"
    module.write_text("def :\n", encoding="utf-8")

    result = _run_configured_pylint(module)

    assert result.returncode != 0, result.stdout
    assert "E0001: Parsing failed" in result.stdout, result.stdout


@pytest.mark.parametrize(
    ("cpus", "expected"),
    [(1, 2), (4, 2), (19, 2), (20, 2), (64, 6), (256, 25)],
)
def test_the_worker_pool_keeps_a_floor_of_two(cpus: int, expected: int) -> None:
    """The pool must stay useful at both ends of the machine range."""
    jobs = _jobs_for(cpus)
    assert jobs == expected, f"{cpus} CPUs must yield {expected} workers, not {jobs}"
