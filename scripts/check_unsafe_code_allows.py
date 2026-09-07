#!/usr/bin/env python3
"""Reject workspace source that suppresses the ``unsafe_code`` lint.

The workspace denies ``unsafe_code``. The audited ``ctor`` macro needs a
macro-generated allowance for its linker-level implementation, but workspace
source must never add that allowance without an explicit policy decision.
"""

import re
import sys
import typing as typ
from pathlib import Path

if typ.TYPE_CHECKING:
    import collections.abc as cabc

EXCLUDED_DIRECTORY_NAMES = frozenset({
    ".git",
    ".mypy_cache",
    ".pytest_cache",
    ".ruff_cache",
    ".venv",
    "node_modules",
    "target",
    "vendor",
})
UNSAFE_CODE_ALLOW = re.compile(r"#!?\s*\[\s*allow\s*\(\s*unsafe_code\s*\)\s*\]")


def iter_workspace_sources(root: Path) -> cabc.Iterator[Path]:
    """Yield workspace-owned Rust sources, excluding generated dependencies."""
    for source in root.rglob("*.rs"):
        relative = source.relative_to(root)
        if EXCLUDED_DIRECTORY_NAMES.isdisjoint(relative.parts):
            yield source


def find_violations(root: Path) -> list[str]:
    """Return workspace source locations that suppress ``unsafe_code``."""
    violations: list[str] = []
    for source in iter_workspace_sources(root):
        relative = source.relative_to(root).as_posix()
        for line_number, line in enumerate(
            source.read_text(encoding="utf-8").splitlines(), 1
        ):
            if match := UNSAFE_CODE_ALLOW.search(line):
                violations.append(f"{relative}:{line_number}: {match.group()}")
    return violations


def report_violations(violations: list[str]) -> int:
    """Report policy violations and return a failing exit status when present."""
    if not violations:
        return 0

    print("workspace source may not suppress unsafe_code:", file=sys.stderr)
    for violation in violations:
        print(f"  {violation}", file=sys.stderr)
    print(
        "unsafe_code exceptions require an explicit policy decision; do not add "
        "#[allow(unsafe_code)] or #![allow(unsafe_code)] to workspace source.",
        file=sys.stderr,
    )
    return 1


def main(root: Path | None = None) -> int:
    """Reject ``unsafe_code`` suppressions beneath ``root`` or this repository."""
    repository_root = root or Path(__file__).resolve().parents[1]
    return report_violations(find_violations(repository_root))


if __name__ == "__main__":
    sys.exit(main())
