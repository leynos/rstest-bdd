#!/usr/bin/env python3
"""Validate that Rust source files stay within the 400 line budget."""

from __future__ import annotations

import sys
from pathlib import Path

MAX_LINES = 400
ALLOWLIST_FILE = "scripts/rs-length-allowlist.txt"


def load_allowlist(root: Path) -> set[Path]:
    """Return the set of allowlisted Rust files relative to the repo root."""
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


def count_file_lines(path: Path) -> int:
    """Count lines in a file, falling back to binary mode on encoding errors."""
    try:
        return sum(1 for _ in path.open(encoding="utf-8"))
    except UnicodeDecodeError:
        return path.read_bytes().count(b"\n") + 1


def collect_violations(root: Path, allowlist: set[Path]) -> list[tuple[Path, int]]:
    """Find all Rust files exceeding MAX_LINES that aren't allowlisted."""
    violations: list[tuple[Path, int]] = []

    for path in root.rglob("*.rs"):
        if "target" in path.parts:
            continue
        rel_path = path.relative_to(root)
        if rel_path in allowlist:
            continue

        line_count = count_file_lines(path)
        if line_count > MAX_LINES:
            violations.append((rel_path, line_count))

    return violations


def report_violations(violations: list[tuple[Path, int]]) -> int:
    """Print violations to stderr and return exit code (1 if any, else 0)."""
    if not violations:
        return 0

    print("Rust sources exceed the 400 line limit:", file=sys.stderr)
    for rel_path, count in sorted(violations):
        print(f"  {rel_path} ({count} lines)", file=sys.stderr)
    print(
        "Update the module layout to split large files or add a temporary entry "
        "to scripts/rs-length-allowlist.txt if the refactor is tracked separately.",
        file=sys.stderr,
    )
    return 1


def main() -> int:
    """Check every Rust file and report ones that exceed the line budget."""
    repo_root = Path(__file__).resolve().parents[1]
    allowlist = load_allowlist(repo_root)

    if exit_code := check_allowlist_integrity(repo_root, allowlist):
        return exit_code

    violations = collect_violations(repo_root, allowlist)
    return report_violations(violations)


if __name__ == "__main__":
    sys.exit(main())
