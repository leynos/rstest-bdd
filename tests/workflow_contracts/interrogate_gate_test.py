"""Verify the Python docstring-coverage gate the Makefile wires into lint.

``make lint`` reaches an ``interrogate`` run through ``lint-python``. The run
must cover every Python file the repository owns at a threshold of 100, with
a guard that fails on any ``MISSED`` row: interrogate rounds its percentage to
one decimal, so the threshold alone passes a lone gap in a large repository.
These contracts read the variables and run the real target against a stub
tool, so a deleted flag, a dropped prerequisite, or a guard that stops failing
breaks a test rather than silently restoring the gap.

Run via ``make test-workflow-contracts``.
"""

import re
import shutil
import stat
import subprocess  # ruff: ignore[suspicious-subprocess-import] - runs the trusted local Makefile.
import textwrap
from pathlib import Path

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
MAKEFILE = REPOSITORY_ROOT / "Makefile"
PRUNED = {".git", "target", ".venv", ".uv-cache", ".uv-tools", "node_modules"}


def _makefile_text() -> str:
    """Return the Makefile source.

    Returns
    -------
    str
        The Makefile text.
    """
    return MAKEFILE.read_text(encoding="utf-8")


def _variable(name: str) -> str:
    """Return the value of a ``NAME ?=`` or ``NAME =`` assignment, joined.

    Parameters
    ----------
    name : str
        The Make variable to read.

    Returns
    -------
    str
        The value with continuation lines joined.
    """
    pattern = re.compile(rf"^{name}\s*\??=\s*((?:.*\\\n)*.*)$", re.MULTILINE)
    match = pattern.search(_makefile_text())
    assert match, f"the Makefile must assign {name}"
    return re.sub(r"\\\n\s*", " ", match.group(1)).strip()


def _prerequisites(target: str) -> list[str]:
    """Return the prerequisites declared for a target.

    Parameters
    ----------
    target : str
        The Make target to read.

    Returns
    -------
    list[str]
        The prerequisite names.
    """
    pattern = re.compile(rf"^{re.escape(target)}:([^=\n]*)$", re.MULTILINE)
    match = pattern.search(_makefile_text())
    assert match, f"the Makefile must declare a {target} target"
    return match.group(1).split("##")[0].split()


def _python_directories() -> set[str]:
    """Return the paths that hold Python source the repository owns.

    Returns
    -------
    set[str]
        Directories (or root-level files) containing ``.py`` files.
    """
    found: set[str] = set()
    for path in REPOSITORY_ROOT.rglob("*.py"):
        relative = path.relative_to(REPOSITORY_ROOT)
        if PRUNED & set(relative.parts):
            continue
        parent = relative.parts[:-1]
        found.add("/".join(parent) if parent else relative.name)
    return found


def test_lint_reaches_interrogate_through_lint_python() -> None:
    """Dropping either link would remove the gate from ``make lint``."""
    assert "interrogate" in _prerequisites("lint-python"), (
        "lint-python must depend on interrogate"
    )
    assert "$(MAKE) lint-python" in _makefile_text(), "lint must call lint-python"


def test_the_tool_is_pinned_and_run_through_uv() -> None:
    """An unpinned tool could change the verdict with no repository change."""
    assert _variable("INTERROGATE_VERSION") == "1.7.0", "interrogate must stay pinned"
    command = _variable("INTERROGATE")
    assert "tool run" in command, "the tool must run through uv tool run"
    assert "--from 'interrogate==$(INTERROGATE_VERSION)'" in command, (
        "the pin must reach the command"
    )


def test_the_flags_demand_a_full_exact_report() -> None:
    """Threshold 100, every definition printed, and no ignore flag."""
    flags = _variable("INTERROGATE_FLAGS").split()
    assert flags[flags.index("--fail-under") + 1] == "100", "the threshold must be 100"
    assert "-vv" in flags, "the report must print every definition"
    relaxing = [
        flag for flag in flags if flag.startswith(("--ignore", "--exclude", "-e"))
    ]
    assert not relaxing, f"no ignore or exclude flag may relax the gate: {relaxing}"


def test_every_python_directory_is_a_target() -> None:
    """A Python directory outside ``INTERROGATE_TARGETS`` is not checked."""
    targets = _variable("INTERROGATE_TARGETS").split()
    uncovered = [
        directory
        for directory in sorted(_python_directories())
        if not any(directory == t or directory.startswith(f"{t}/") for t in targets)
    ]
    assert not uncovered, f"add these to INTERROGATE_TARGETS: {uncovered}"


def _run_target(tmp_path: Path, script: str) -> subprocess.CompletedProcess[str]:
    """Run ``make interrogate`` with the tool replaced by a stub script.

    Parameters
    ----------
    tmp_path : Path
        A scratch directory for the stub.
    script : str
        The stub's shell body.

    Returns
    -------
    subprocess.CompletedProcess[str]
        The result, with both streams captured as text.
    """
    stub = tmp_path / "stub.sh"
    stub.write_text(
        '#!/bin/sh\nprintf "%s\\n" "$@" > "$(dirname "$0")/args.txt"\n'
        + textwrap.dedent(script),
        encoding="utf-8",
    )
    stub.chmod(stub.stat().st_mode | stat.S_IXUSR)
    make = shutil.which("make")
    assert make is not None, "make must be available to run the gate"
    return subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - arguments built by this test.
        [make, "--no-print-directory", "interrogate", f"INTERROGATE={stub}"],
        cwd=REPOSITORY_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )


def test_the_target_passes_when_the_tool_reports_no_gap(tmp_path: Path) -> None:
    """A clean report and a zero status pass, and the report reaches stdout."""
    result = _run_target(tmp_path, "echo '| f.py | COVERED |'\nexit 0\n")
    assert result.returncode == 0, result.stdout + result.stderr
    assert "| f.py | COVERED |" in result.stdout, "the report must be printed"


def test_the_target_passes_the_configured_flags_and_targets(tmp_path: Path) -> None:
    """The tool receives exactly ``INTERROGATE_FLAGS`` then ``INTERROGATE_TARGETS``."""
    _run_target(tmp_path, "exit 0\n")
    received = (tmp_path / "args.txt").read_text(encoding="utf-8").split()
    expected = (
        _variable("INTERROGATE_FLAGS").split()
        + _variable("INTERROGATE_TARGETS").split()
    )
    assert received == expected, f"the tool received {received}, not {expected}"


def test_the_target_fails_when_the_tool_fails(tmp_path: Path) -> None:
    """The tool's own non-zero status is kept."""
    result = _run_target(tmp_path, "echo 'FAILED'\nexit 1\n")
    assert result.returncode != 0, "a failing tool status must fail the target"


def test_the_target_fails_on_a_missed_row_the_percentage_hid(tmp_path: Path) -> None:
    """Rounded to 100.0% with exit 0, a MISSED row still fails the target."""
    result = _run_target(
        tmp_path, "echo '| f.main (L3) |  MISSED |'\necho 'RESULT: PASSED'\nexit 0\n"
    )
    assert result.returncode != 0, "a MISSED row must fail the target"
    assert "undocumented definitions remain" in result.stderr, result.stderr
