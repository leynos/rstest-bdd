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


def is_scannable_file(path: Path) -> bool:
    """Return ``True`` when *path* is a file with a supported extension.

    Examples
    --------
    >>> is_scannable_file(Path("src/lib.rs"))
    True
    >>> is_scannable_file(Path("README.md"))
    False
    """
    return path.is_file() and path.suffix in SCAN_EXTENSIONS


def is_cargo_manifest(path: Path) -> bool:
    """Return ``True`` for Cargo manifests that should be inspected.

    Examples
    --------
    >>> is_cargo_manifest(Path("crate/Cargo.toml"))
    True
    >>> is_cargo_manifest(Path("crate/Other.toml"))
    False
    """
    return path.suffix == ".toml" and path.name == "Cargo.toml"


def is_cargo_lockfile(path: Path) -> bool:
    """Return ``True`` for Cargo.lock files worth checking.

    Examples
    --------
    >>> is_cargo_lockfile(Path("Cargo.lock"))
    True
    >>> is_cargo_lockfile(Path("not-a-lock.lock"))
    False
    """
    return path.suffix == ".lock" and path.name == "Cargo.lock"


def is_in_skipped_directory(path: Path) -> bool:
    """Return ``True`` when any path segment should be skipped.

    Examples
    --------
    >>> is_in_skipped_directory(Path("target/debug/lib.rs"))
    True
    >>> is_in_skipped_directory(Path("src/lib.rs"))
    False
    """
    return any(part in SKIP_DIRS for part in path.parts)


def should_include_file(path: Path) -> bool:
    """Return ``True`` when *path* satisfies all scanning criteria.

    Examples
    --------
    >>> should_include_file(Path("src/lib.rs"))
    True
    >>> should_include_file(Path("docs/guide.md"))
    False
    >>> should_include_file(Path("Cargo.lock"))
    True
    """
    if not is_scannable_file(path):
        return False
    if path.suffix == ".toml" and not is_cargo_manifest(path):
        return False
    if path.suffix == ".lock" and not is_cargo_lockfile(path):
        return False
    return not is_in_skipped_directory(path)


def iter_candidate_files(root: Path) -> typ.Iterator[Path]:
    """Return paths beneath *root* that should be scanned for async-trait usage.

    Parameters
    ----------
    root : Path
        Directory from which to traverse the repository tree recursively.

    Yields
    ------
    Path
        Each file meeting the scanning criteria, including manifests and
        lockfiles subject to additional validation.
    """
    for path in root.rglob("*"):
        if should_include_file(path):
            yield path


def line_comment_precedes_block_comment(
    line_comment_pos: int, block_comment_pos: int
) -> bool:
    """Return ``True`` when line comments outrank block comments for precedence.

    Line comments win whenever they appear at all and either no block comment is
    present or the line comment is encountered first. This mirrors the parser's
    behaviour where everything to the right of ``//`` becomes a comment.

    Examples
    --------
    >>> line_comment_precedes_block_comment(3, -1)
    True
    >>> line_comment_precedes_block_comment(-1, 5)
    False
    >>> line_comment_precedes_block_comment(4, 10)
    True
    """
    line_comment_exists = line_comment_pos != -1
    no_block_comment = block_comment_pos == -1
    line_comment_comes_first = line_comment_pos < block_comment_pos
    return line_comment_exists and (no_block_comment or line_comment_comes_first)


def handle_block_comment_continuation(line: str, cursor: int) -> tuple[bool, int, bool]:
    """Handle case when already inside a block comment."""
    block_end = line.find("*/", cursor)
    if block_end == -1:
        return (False, len(line) + 1, True)
    return (False, block_end + 2, False)


def handle_line_comment_section(
    line: str, cursor: int, start_comment: int
) -> tuple[bool, int, bool]:
    """Handle case when a line comment is encountered first."""
    search_area = line[cursor:start_comment]
    pattern_found = bool(ASYNC_TRAIT_PATTERN.search(search_area))
    return (pattern_found, len(line) + 1, False)


def handle_block_comment_start(
    line: str, cursor: int, start_block: int
) -> tuple[bool, int, bool]:
    """Handle case when a block comment starts."""
    search_area = line[cursor:start_block]
    pattern_found = bool(ASYNC_TRAIT_PATTERN.search(search_area))
    if pattern_found:
        return (True, start_block + 2, True)
    return (False, start_block + 2, True)


def handle_plain_code(line: str, cursor: int) -> tuple[bool, int, bool]:
    """Handle case when remaining line has no comments."""
    search_area = line[cursor:]
    pattern_found = bool(ASYNC_TRAIT_PATTERN.search(search_area))
    return (pattern_found, len(line) + 1, False)


def process_line_for_async_trait(
    line: str,
    in_block_comment: bool,  # noqa: FBT001 - lint override for mandated signature
) -> tuple[bool, bool]:
    """Process a line to detect ``async-trait`` usage outside of comments.

    Parameters
    ----------
    line : str
        The line of code to process
    in_block_comment : bool
        Whether we are currently inside a block comment from the previous line

    Returns
    -------
    tuple[bool, bool]
        ``(pattern_found, still_in_block_comment)``

    Examples
    --------
    >>> process_line_for_async_trait("use async_trait::async_trait;", False)
    (True, False)
    >>> process_line_for_async_trait("// async_trait comment", False)
    (False, False)
    """
    cursor = 0
    current_block_state = in_block_comment

    while cursor <= len(line):
        if current_block_state:
            found, cursor, current_block_state = handle_block_comment_continuation(
                line, cursor
            )
            if found:
                return (True, current_block_state)
            continue

        start_block = line.find("/*", cursor)
        start_comment = line.find("//", cursor)

        if line_comment_precedes_block_comment(start_comment, start_block):
            found, cursor, current_block_state = handle_line_comment_section(
                line, cursor, start_comment
            )
        elif start_block != -1:
            found, cursor, current_block_state = handle_block_comment_start(
                line, cursor, start_block
            )
        else:
            found, cursor, current_block_state = handle_plain_code(line, cursor)

        if found:
            return (True, current_block_state)

    return (False, current_block_state)


def find_async_trait_in_rust(path: Path) -> list[int]:
    """Return the 1-based line numbers where the symbol appears in code."""
    offences: list[int] = []
    try:
        contents = path.read_text(encoding="utf-8")
    except UnicodeDecodeError:
        return offences

    in_block_comment = False
    for line_no, line in enumerate(contents.splitlines(), start=1):
        found, in_block_comment = process_line_for_async_trait(line, in_block_comment)
        if found:
            offences.append(line_no)
    return offences


def manifest_declares_async_trait(path: Path) -> bool:
    """Return ``True`` when *path* declares the forbidden dependency."""
    try:
        data = tomllib.loads(path.read_text(encoding="utf-8"))
    except (UnicodeDecodeError, tomllib.TOMLDecodeError):
        return False

    def is_dependencies_section_with_async_trait(key: str, value: object) -> bool:
        """Return ``True`` when *value* references async-trait in dependency tables.

        Examples
        --------
        >>> is_dependencies_section_with_async_trait(
        ...     "dependencies",
        ...     {"async-trait": "1"},
        ... )
        True
        >>> is_dependencies_section_with_async_trait("package", {})
        False
        >>> is_dependencies_section_with_async_trait("dev-dependencies", [])
        False
        """
        if not key.endswith("dependencies"):
            return False
        if not isinstance(value, dict):
            return False
        return "async-trait" in value

    def visit_dict(node: dict) -> bool:
        """Return ``True`` when *node* contains the forbidden dependency."""
        for key, value in node.items():
            if is_dependencies_section_with_async_trait(key, value):
                return True
            if visit(value):
                return True
        return False

    def visit_list(node: list) -> bool:
        """Return ``True`` when any entry in *node* references async-trait."""
        return any(visit(item) for item in node)

    def visit(node: object) -> bool:
        if isinstance(node, dict):
            return visit_dict(node)
        if isinstance(node, list):
            return visit_list(node)
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
