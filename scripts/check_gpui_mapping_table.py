#!/usr/bin/env python3
"""
Validate that the duplicated GPUI mapping tables stay in step.

The users' guide and the design document both explain how the vendored
``gpui 0.2.2`` shim differs from the published crate with the same version.
This script catches doc-vs-doc drift between those two copies. It deliberately
does not prove either table against the real published ``gpui`` API: local
workspace builds use ``vendor/gpui`` through a path dependency, so that external
surface is checked during release by ``lading publish`` after the staged
workspace strips local patch entries.

Usage
-----
python3 scripts/check_gpui_mapping_table.py

Exit codes
----------
0
    The four mapping-table data rows match after whitespace normalization.
1
    A table is missing, malformed, or the two table bodies differ.
"""

import re
import sys
from pathlib import Path

DESIGN_DOC = Path("docs/rstest-bdd-design.md")
USERS_GUIDE = Path("docs/users-guide.md")

DESIGN_HEADING = "Interim GPUI state pattern"
USERS_HEADING = "Stateful GPUI scenarios with durable handles"
TABLE_HEADER = "> | Operation |"
TABLE_SEPARATOR = "> | --- |"
EXPECTED_DATA_ROWS = 4


class MappingTableError(ValueError):
    """A GPUI mapping table is missing or malformed."""

    @staticmethod
    def heading_not_found_message(heading: str) -> str:
        """Return the message for a missing anchor heading."""
        return f"heading not found: {heading}"

    @staticmethod
    def separator_not_found_message(heading: str) -> str:
        """Return the message for a table without its separator row."""
        return f"mapping table under {heading!r} has no separator row"

    @staticmethod
    def wrong_row_count_message(heading: str, actual: int) -> str:
        """Return the message for an unexpected number of data rows."""
        return (
            f"mapping table under {heading!r} has {actual} data rows; "
            f"expected {EXPECTED_DATA_ROWS}"
        )

    @staticmethod
    def table_not_found_message(heading: str) -> str:
        """Return the message for a missing mapping table."""
        return f"mapping table not found under heading: {heading}"

    @staticmethod
    def document_unreadable_message(relative_path: Path, error: OSError) -> str:
        """Return the message for a document that could not be read."""
        return f"could not read {relative_path}: {error}"


def normalize_table_row(row: str) -> str:
    """
    Collapse insignificant spacing in a Markdown table row.

    Parameters
    ----------
    row : str
        A raw Markdown table row, including any blockquote marker.

    Returns
    -------
    str
        The row with internal whitespace runs collapsed to single spaces.
    """
    return re.sub(r"\s+", " ", row).strip()


def _collect_section_lines(
    lines: list[str], start: int, peer_pattern: re.Pattern[str]
) -> list[str]:
    """Collect lines from *start* until a same-or-higher-level heading."""
    section: list[str] = []
    for candidate in lines[start:]:
        if peer_pattern.match(candidate):
            break
        section.append(candidate)
    return section


def find_section_after_heading(markdown: str, heading: str) -> list[str] | None:
    """
    Return the lines after a named heading and before the next same-level peer.

    Parameters
    ----------
    markdown : str
        Full document content.
    heading : str
        Heading text without leading ``#`` markers.

    Returns
    -------
    list[str] | None
        Section lines when found; otherwise ``None``.
    """
    lines = markdown.splitlines()
    heading_pattern = re.compile(
        rf"^(?P<level>#+)\s+(?P<text>.*{re.escape(heading)})\s*$"
    )

    for index, line in enumerate(lines):
        match = heading_pattern.match(line)
        if match is None:
            continue
        level = len(match.group("level"))
        peer_pattern = re.compile(rf"^#{{1,{level}}}\s+")
        return _collect_section_lines(lines, index + 1, peer_pattern)

    return None


def _collect_table_rows(section: list[str], start: int) -> list[str]:
    """Return normalized data rows from *start*, stopping at the first non-row line."""
    rows: list[str] = []
    for row in section[start:]:
        if not row.startswith("> |"):
            break
        rows.append(normalize_table_row(row))
    return rows


def _parse_table_at(section: list[str], header_index: int, heading: str) -> list[str]:
    """Validate the separator row and return the validated data rows."""
    separator_index = header_index + 1
    if separator_index >= len(section) or not section[separator_index].startswith(
        TABLE_SEPARATOR
    ):
        message = MappingTableError.separator_not_found_message(heading)
        raise MappingTableError(message)
    rows = _collect_table_rows(section, separator_index + 1)
    if len(rows) != EXPECTED_DATA_ROWS:
        message = MappingTableError.wrong_row_count_message(heading, len(rows))
        raise MappingTableError(message)
    return rows


def extract_mapping_rows(markdown: str, heading: str) -> list[str]:
    """
    Extract normalized GPUI mapping-table data rows from one document.

    Parameters
    ----------
    markdown : str
        Full document content.
    heading : str
        Section heading that anchors the relevant table.

    Returns
    -------
    list[str]
        The four ordered data rows, normalized for whitespace.

    Raises
    ------
    MappingTableError
        The requested heading or mapping table is absent, the mapping table
        separator is missing, or the table has the wrong number of data rows.
    """
    section = find_section_after_heading(markdown, heading)
    if section is None:
        message = MappingTableError.heading_not_found_message(heading)
        raise MappingTableError(message)

    for index, line in enumerate(section):
        if line.startswith(TABLE_HEADER):
            return _parse_table_at(section, index, heading)

    message = MappingTableError.table_not_found_message(heading)
    raise MappingTableError(message)


def read_mapping_rows(root: Path, relative_path: Path, heading: str) -> list[str]:
    """
    Read one document and extract its GPUI mapping table rows.

    Parameters
    ----------
    root : Path
        Repository root directory.
    relative_path : Path
        Document path relative to ``root``.
    heading : str
        Section heading that anchors the relevant table.

    Returns
    -------
    list[str]
        Normalized data rows.

    Raises
    ------
    MappingTableError
        The document cannot be read, or its mapping-table heading, table,
        separator, or data-row count is invalid.
    """
    path = root / relative_path
    try:
        markdown = path.read_text(encoding="utf-8")
    except OSError as err:
        message = MappingTableError.document_unreadable_message(relative_path, err)
        raise MappingTableError(message) from err
    return extract_mapping_rows(markdown, heading)


def check_mapping_tables(root: Path) -> list[str]:
    """
    Check that the users' guide and design mapping tables match.

    Parameters
    ----------
    root : Path
        Repository root directory.

    Returns
    -------
    list[str]
        Human-readable violations; empty when the tables match.
    """
    try:
        design_rows = read_mapping_rows(root, DESIGN_DOC, DESIGN_HEADING)
        users_rows = read_mapping_rows(root, USERS_GUIDE, USERS_HEADING)
    except ValueError as err:
        return [str(err)]

    if design_rows == users_rows:
        return []

    violations = ["GPUI mapping table data rows differ:"]
    for index, (design_row, users_row) in enumerate(
        zip(design_rows, users_rows, strict=True), start=1
    ):
        if design_row != users_row:
            violations.extend([
                f"row {index}:",
                f"  {DESIGN_DOC}: {design_row}",
                f"  {USERS_GUIDE}: {users_row}",
            ])
            break
    return violations


def main() -> int:
    """Check the duplicated GPUI mapping tables and report violations."""
    root = Path(__file__).resolve().parents[1]
    violations = check_mapping_tables(root)
    for violation in violations:
        print(violation, file=sys.stderr)
    return 1 if violations else 0


if __name__ == "__main__":
    sys.exit(main())
