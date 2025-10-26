#!/usr/bin/env python3
"""Fail the build when `async-trait` sneaks back into the tree.

The project deliberately avoids the crate so trait objects stay zero-cost and
stack traces remain readable. This script runs in CI (and can be executed
locally) to catch the dependency reappearing either in `Cargo.toml` files or in
Rust sources.
"""

from __future__ import annotations

import sys
import typing as typ
from pathlib import Path

# Restrict the scan to files whose extensions could legitimately reference the
# crate. This avoids doc files where the name may be mentioned in prose.
SCAN_EXTENSIONS = {".rs", ".toml", ".lock"}
# Skip generated or third-party directories.
SKIP_DIRS = {".git", "target", "node_modules", "docs"}

PATTERNS = {
    ".rs": ("async_trait",),
    ".toml": ("async-trait",),
    ".lock": ("async-trait",),
}


def iter_candidate_files(root: Path) -> typ.Iterator[Path]:
    """Yield files under *root* whose suffix is in ``SCAN_EXTENSIONS``."""
    for path in root.rglob("*"):
        if not path.is_file():
            continue
        if path.suffix not in SCAN_EXTENSIONS:
            continue
        if any(part in SKIP_DIRS for part in path.parts):
            continue
        yield path


def find_violations(root: Path) -> list[str]:
    """Return a list describing where forbidden patterns appear."""
    problems: list[str] = []
    for file_path in iter_candidate_files(root):
        patterns = PATTERNS.get(file_path.suffix, ())
        try:
            contents = file_path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            # Binary or non-UTF8 file; skip it defensively.
            continue
        for line_no, line in enumerate(contents.splitlines(), start=1):
            matches = [
                f"{file_path.relative_to(root)}:{line_no}: contains '{pattern}'"
                for pattern in patterns
                if pattern in line
            ]
            problems.extend(matches)
    return problems


def main() -> int:
    """Check the repository for forbidden async-trait references."""
    root = Path(__file__).resolve().parents[1]
    violations = find_violations(root)
    if violations:
        heading = "async-trait usage is forbidden; remove the dependency and macros"
        print(heading, file=sys.stderr)
        for entry in violations:
            print(entry, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
