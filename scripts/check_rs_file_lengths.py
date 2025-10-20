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


def main() -> int:
    """Check every Rust file and report ones that exceed the line budget."""
    repo_root = Path(__file__).resolve().parents[1]
    allowlist = load_allowlist(repo_root)

    missing_allowlisted = [
        path for path in allowlist if not (repo_root / path).exists()
    ]
    if missing_allowlisted:
        for path in missing_allowlisted:
            print(f"allowlist entry no longer exists: {path}", file=sys.stderr)
        return 1

    failures: list[tuple[Path, int]] = []

    for path in repo_root.rglob("*.rs"):
        if "target" in path.parts:
            continue
        rel_path = path.relative_to(repo_root)
        if rel_path in allowlist:
            continue

        try:
            line_count = sum(1 for _ in path.open(encoding="utf-8"))
        except UnicodeDecodeError:
            # Fall back to binary mode if a file uses a different encoding.
            line_count = path.read_bytes().count(b"\n") + 1

        if line_count > MAX_LINES:
            failures.append((rel_path, line_count))

    if failures:
        print("Rust sources exceed the 400 line limit:", file=sys.stderr)
        for rel_path, count in sorted(failures):
            print(f"  {rel_path} ({count} lines)", file=sys.stderr)
        print(
            "Update the module layout to split large files or add a temporary entry "
            "to scripts/rs-length-allowlist.txt if the refactor is tracked separately.",
            file=sys.stderr,
        )
        return 1

    return 0


if __name__ == "__main__":
    sys.exit(main())
