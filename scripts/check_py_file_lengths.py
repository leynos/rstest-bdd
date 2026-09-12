#!/usr/bin/env python3
"""
Validate that Python modules stay within the 400-line budget.

This script enforces a maximum line count (``MAX_LINES = 400``) on every
Python module beneath the lint roots, excluding paths listed in the
allowlist file (``scripts/py-length-allowlist.txt``). It is invoked by the
``make lint`` target with the same roots as the PyPy-backed PyLint pass.

Measuring the files rather than their parse trees is the point. Managed
PyPy lags the CPython 3.14 syntax this project targets, so a module using
newer syntax produces no PyLint output at all -- including the
``too-many-lines`` message that would otherwise report it. Counting lines
from the bytes means every module is measured, whether or not a linter can
parse it, and a root or file that cannot be read is reported instead of
quietly dropped from the scan.

Usage
-----
python3 scripts/check_py_file_lengths.py [root ...]

Exit codes
----------
0
    All Python modules comply with the line limit.
1
    Violations found, a lint root is absent, or the allowlist references a
    path that no longer exists.

The allowlist supports comments (lines starting with ``#``) and empty lines.
A module over the budget should be split rather than allowlisted unless the
refactor is tracked separately.
"""

import sys
import typing as typ
from pathlib import Path

if typ.TYPE_CHECKING:
    import collections.abc as cabc

MAX_LINES = 400
ALLOWLIST_FILE = "scripts/py-length-allowlist.txt"
DEFAULT_LINT_ROOTS = ("scripts", "tests/workflow_contracts")
# Directories that never hold first-party modules. The lint roots are
# hand-picked, so this only guards against a stray virtual environment or
# build directory captured inside one of them.
EXCLUDED_DIRECTORIES = frozenset({
    ".git",
    ".venv",
    "__pycache__",
    "node_modules",
    "target",
    "venv",
})


def load_allowlist(root: Path) -> set[Path]:
    """
    Load the allowlist of modules exempt from the line limit.

    Reads the allowlist file (``scripts/py-length-allowlist.txt``) and
    returns a set of ``Path`` objects relative to the repository root. Lines
    starting with ``#`` and empty lines are ignored.

    Parameters
    ----------
    root : Path
        The repository root directory.

    Returns
    -------
    set[Path]
        Set of allowlisted file paths relative to ``root``. Returns an empty
        set if the allowlist file does not exist.
    """
    allowlist_path = root / ALLOWLIST_FILE
    if not allowlist_path.exists():
        return set()

    entries: set[Path] = set()
    for line in allowlist_path.read_text(encoding="utf-8").splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        entries.add(Path(stripped))
    return entries


def check_allowlist_integrity(root: Path, allowlist: set[Path]) -> int:
    """Verify all allowlisted paths exist. Return 1 if any are missing, else 0."""
    missing = [path for path in allowlist if not (root / path).exists()]
    if missing:
        for path in missing:
            print(f"allowlist entry no longer exists: {path}", file=sys.stderr)
        return 1
    return 0


def check_lint_roots(root: Path, lint_roots: cabc.Sequence[str]) -> int:
    """Verify every lint root is a directory. Return 1 if any are missing, else 0."""
    missing = [name for name in lint_roots if not (root / name).is_dir()]
    if missing:
        for name in missing:
            print(f"lint root is not a directory: {name}", file=sys.stderr)
        return 1
    return 0


def count_file_lines(path: Path) -> int:
    """Count lines in a module, falling back to binary mode on encoding errors."""
    try:
        return sum(1 for _ in path.open(encoding="utf-8"))
    except UnicodeDecodeError:
        content = path.read_bytes()
        if not content:
            return 0
        newline_count = content.count(b"\n")
        return newline_count if content.endswith(b"\n") else newline_count + 1


def discover_modules(root: Path, lint_roots: cabc.Sequence[str]) -> cabc.Iterator[Path]:
    """Yield every module beneath the lint roots, relative to *root*."""
    for name in lint_roots:
        for path in sorted((root / name).rglob("*.py")):
            if EXCLUDED_DIRECTORIES.intersection(path.parts):
                continue
            yield path.relative_to(root)


def collect_violations(
    root: Path, lint_roots: cabc.Sequence[str], allowlist: set[Path]
) -> list[tuple[Path, int]]:
    """Find all modules exceeding MAX_LINES that aren't allowlisted."""
    violations: list[tuple[Path, int]] = []

    for rel_path in discover_modules(root, lint_roots):
        if rel_path in allowlist:
            continue

        line_count = count_file_lines(root / rel_path)
        if line_count > MAX_LINES:
            violations.append((rel_path, line_count))

    return violations


def report_violations(violations: list[tuple[Path, int]]) -> int:
    """Print violations to stderr and return exit code (1 if any, else 0)."""
    if not violations:
        return 0

    print(f"Python modules exceed the {MAX_LINES} line limit:", file=sys.stderr)
    for rel_path, count in sorted(violations):
        print(f"  {rel_path} ({count} lines)", file=sys.stderr)
    print(
        "Split the module or add a temporary entry to "
        f"{ALLOWLIST_FILE} if the refactor is tracked separately. The count "
        "comes from the file itself, so a module no parser accepts is still "
        "measured here.",
        file=sys.stderr,
    )
    return 1


def main(argv: cabc.Sequence[str]) -> int:
    """Check every module beneath the lint roots against the line budget."""
    repo_root = Path(__file__).resolve().parents[1]
    lint_roots = tuple(argv) or DEFAULT_LINT_ROOTS
    allowlist = load_allowlist(repo_root)

    if exit_code := check_allowlist_integrity(repo_root, allowlist):
        return exit_code
    if exit_code := check_lint_roots(repo_root, lint_roots):
        return exit_code

    violations = collect_violations(repo_root, lint_roots, allowlist)
    return report_violations(violations)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
