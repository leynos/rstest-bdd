#!/usr/bin/env python3
"""Fail the build when ``async-trait`` sneaks back into the tree."""

from __future__ import annotations

import re
import sys
import tomllib
import typing as typ
from pathlib import Path

if typ.TYPE_CHECKING:
    import collections.abc as cabc

# Restrict the scan to files whose extensions could legitimately reference the
# crate. This avoids doc files where the name may be mentioned in prose.
SCAN_EXTENSIONS = {".rs", ".toml", ".lock"}
# Skip generated or third-party directories.
SKIP_DIRS = {".git", "target", "node_modules", "docs"}

ASYNC_TRAIT_PATTERN = re.compile(r"\basync[-_]trait\b")
LOCKFILE_PATTERN = re.compile(r'^\s*name\s*=\s*"async-trait"$', re.MULTILINE)


def is_scannable_file(path: Path) -> bool:
    """Check whether *path* has a supported extension."""
    return path.is_file() and path.suffix in SCAN_EXTENSIONS


def path_has_name_and_suffix(path: Path, *, name: str, suffix: str) -> bool:
    """Return ``True`` when *path* matches the expected filename metadata."""
    return path.suffix == suffix and path.name == name


def is_cargo_manifest(path: Path) -> bool:
    """Check whether *path* refers to a Cargo manifest."""
    return path_has_name_and_suffix(path, name="Cargo.toml", suffix=".toml")


def is_cargo_lockfile(path: Path) -> bool:
    """Check whether *path* refers to a Cargo lockfile."""
    return path_has_name_and_suffix(path, name="Cargo.lock", suffix=".lock")


def is_in_skipped_directory(path: Path) -> bool:
    """Check whether *path* lies beneath a skipped directory."""
    return any(part in SKIP_DIRS for part in path.parts)


def should_include_file(path: Path) -> bool:
    """Determine whether *path* qualifies for scanning."""
    if not is_scannable_file(path):
        return False
    if path.suffix == ".toml" and not is_cargo_manifest(path):
        return False
    if path.suffix == ".lock" and not is_cargo_lockfile(path):
        return False
    return not is_in_skipped_directory(path)


def iter_candidate_files(root: Path) -> cabc.Iterator[Path]:
    """Return paths beneath *root* that should be scanned for async-trait usage."""
    for path in root.rglob("*"):
        if should_include_file(path):
            yield path


def line_comment_precedes_block_comment(
    line_comment_pos: int, block_comment_pos: int
) -> bool:
    """Determine precedence between line and block comments on the same line."""
    line_comment_exists = line_comment_pos != -1
    no_block_comment = block_comment_pos == -1
    line_comment_comes_first = line_comment_pos < block_comment_pos
    return line_comment_exists and (no_block_comment or line_comment_comes_first)


def handle_block_comment_continuation(line: str, cursor: int) -> tuple[bool, int, bool]:
    """Process a line whilst inside a block comment."""
    block_end = line.find("*/", cursor)
    if block_end == -1:
        return (False, len(line) + 1, True)
    return (False, block_end + 2, False)


def handle_line_comment_section(
    line: str, cursor: int, start_comment: int
) -> tuple[bool, int, bool]:
    """Process a line where a line comment appears first."""
    search_area = line[cursor:start_comment]
    pattern_found = bool(ASYNC_TRAIT_PATTERN.search(search_area))
    return (pattern_found, len(line) + 1, False)


def handle_block_comment_start(
    line: str, cursor: int, start_block: int
) -> tuple[bool, int, bool]:
    """Process a line where a block comment begins."""
    search_area = line[cursor:start_block]
    pattern_found = bool(ASYNC_TRAIT_PATTERN.search(search_area))
    if pattern_found:
        return (True, start_block + 2, True)
    return (False, start_block + 2, True)


def handle_plain_code(line: str, cursor: int) -> tuple[bool, int, bool]:
    """Process a line with no comments."""
    search_area = line[cursor:]
    pattern_found = bool(ASYNC_TRAIT_PATTERN.search(search_area))
    return (pattern_found, len(line) + 1, False)


def process_line_for_async_trait(
    line: str,
    in_block_comment: bool,  # noqa: FBT001 FIXME: signature mandated by design
) -> tuple[bool, bool]:
    """Process a line to detect ``async-trait`` usage outside of comments."""
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
    """Find lines where ``async-trait`` appears in Rust code."""
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


def is_dependencies_section_with_async_trait(key: str, value: object) -> bool:
    """Check whether a TOML table references ``async-trait``."""
    if not key.endswith("dependencies"):
        return False
    if not isinstance(value, dict):
        return False
    return "async-trait" in value


def toml_tree_declares_async_trait(node: object) -> bool:
    """Return ``True`` when a TOML tree contains an ``async-trait`` dependency."""
    if isinstance(node, dict):
        return any(
            isinstance(key, str)
            and (
                is_dependencies_section_with_async_trait(key, value)
                or toml_tree_declares_async_trait(value)
            )
            for key, value in node.items()
        )
    if isinstance(node, list):
        return any(toml_tree_declares_async_trait(item) for item in node)
    return False


def manifest_declares_async_trait(path: Path) -> bool:
    """Check whether a Cargo manifest declares ``async-trait``."""
    try:
        data = tomllib.loads(path.read_text(encoding="utf-8"))
    except (UnicodeDecodeError, tomllib.TOMLDecodeError, OSError):
        return False
    return toml_tree_declares_async_trait(data)


def lockfile_mentions_async_trait(path: Path) -> bool:
    """Check whether a Cargo lockfile references ``async-trait``."""
    try:
        contents = path.read_text(encoding="utf-8")
    except (UnicodeDecodeError, OSError):
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
