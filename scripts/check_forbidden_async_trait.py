#!/usr/bin/env python3
"""Fail the build when `async-trait` sneaks back into the tree.

The project deliberately avoids the crate so trait objects stay zero-cost and
stack traces remain readable. This script runs in CI (and can be executed
locally) to catch the dependency reappearing either in `Cargo.toml` files or in
Rust sources.
"""

from __future__ import annotations

import re
import sys
import typing as typ
from pathlib import Path

import tomllib

# Restrict the scan to files whose extensions could legitimately reference the
# crate. This avoids doc files where the name may be mentioned in prose.
SCAN_EXTENSIONS = {".rs", ".toml", ".lock"}
# Skip generated or third-party directories.
SKIP_DIRS = {".git", "target", "node_modules", "docs"}

ASYNC_TRAIT_PATTERN = re.compile(r"\basync[-_]trait\b")
LOCKFILE_PATTERN = re.compile(r'^name = "async-trait"$', re.MULTILINE)


def iter_candidate_files(root: Path) -> typ.Iterator[Path]:
    """Yield files under *root* whose suffix is in ``SCAN_EXTENSIONS``."""
    for path in root.rglob("*"):
        if not path.is_file():
            continue
        if path.suffix not in SCAN_EXTENSIONS:
            continue
        if path.suffix == ".toml" and path.name != "Cargo.toml":
            continue
        if path.suffix == ".lock" and path.name != "Cargo.lock":
            continue
        if all(part not in SKIP_DIRS for part in path.parts):
            yield path


def find_async_trait_in_rust(path: Path) -> list[int]:
    """Return the 1-based line numbers where the symbol appears in code."""
    offences: list[int] = []
    in_block_comment = False
    try:
        contents = path.read_text(encoding="utf-8")
    except UnicodeDecodeError:
        return offences

    for line_no, raw_line in enumerate(contents.splitlines(), start=1):
        line = raw_line
        cursor = 0
        search_area = ""
        while cursor <= len(line):
            if in_block_comment:
                block_end = line.find("*/", cursor)
                if block_end == -1:
                    cursor = len(line) + 1
                    break
                cursor = block_end + 2
                in_block_comment = False
                continue
            start_block = line.find("/*", cursor)
            start_comment = line.find("//", cursor)
            if start_comment != -1 and (
                start_block == -1 or start_comment < start_block
            ):
                search_area = line[cursor:start_comment]
                cursor = len(line) + 1
            elif start_block != -1:
                search_area = line[cursor:start_block]
                cursor = start_block + 2
                in_block_comment = True
            else:
                search_area = line[cursor:]
                cursor = len(line) + 1
            if ASYNC_TRAIT_PATTERN.search(search_area):
                offences.append(line_no)
                break
    return offences


def manifest_declares_async_trait(path: Path) -> bool:
    """Return ``True`` when *path* declares the forbidden dependency."""
    try:
        data = tomllib.loads(path.read_text(encoding="utf-8"))
    except (UnicodeDecodeError, tomllib.TOMLDecodeError):
        return False

    def visit(node: object) -> bool:
        if isinstance(node, dict):
            for key, value in node.items():
                if (
                    key.endswith("dependencies")
                    and isinstance(value, dict)
                    and ("async-trait" in value)
                ):
                    return True
                if visit(value):
                    return True
        elif isinstance(node, list):
            return any(visit(item) for item in node)
        return False

    return visit(data)


def lockfile_mentions_async_trait(path: Path) -> bool:
    """Return ``True`` when *path*'s contents reference the crate."""
    try:
        contents = path.read_text(encoding="utf-8")
    except UnicodeDecodeError:
        return False
    return bool(LOCKFILE_PATTERN.search(contents))


def find_violations(root: Path) -> list[str]:
    """Return a list describing where forbidden patterns appear."""
    problems: list[str] = []
    for file_path in iter_candidate_files(root):
        relative = file_path.relative_to(root)
        if file_path.suffix == ".rs":
            problems.extend(
                f"{relative}:{line_no}: contains forbidden async-trait usage"
                for line_no in find_async_trait_in_rust(file_path)
            )
        elif file_path.suffix == ".toml" and manifest_declares_async_trait(file_path):
            problems.append(f"{relative}: declares async-trait dependency")
        elif file_path.suffix == ".lock" and lockfile_mentions_async_trait(file_path):
            problems.append(f"{relative}: references async-trait in lockfile")
    return problems


def main() -> int:
    """Check the repository for forbidden async-trait references."""
    root = Path(__file__).resolve().parents[1]
    if violations := find_violations(root):
        heading = "async-trait usage is forbidden; remove the dependency and macros"
        print(heading, file=sys.stderr)
        for entry in violations:
            print(entry, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
