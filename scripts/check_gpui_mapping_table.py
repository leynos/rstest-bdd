#!/usr/bin/env python3
"""
Validate that the duplicated GPUI mapping tables stay in step.

The users' guide and the design document both explain how the vendored
``gpui 0.2.2`` shim differs from the published crate with the same version.
This script catches doc-vs-doc drift between those two copies. It deliberately
does not prove either table against the real published ``gpui`` API: local
workspace builds use ``vendor/gpui`` through a path dependency, so that external
surface is checked during release by ``scripts/publish_check_gpui*.py``.

Usage
-----
python3 scripts/check_gpui_mapping_table.py

Exit codes
----------
0
    The four mapping-table data rows match after whitespace normalisation.
1
    A table is missing, malformed, or the two table bodies differ.
"""

from __future__ import annotations

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

    @classmethod
    def heading_not_found(cls, heading: str) -> MappingTableError:
        """Build an error for a missing anchor heading."""
        message = f"heading not found: {heading}"
        return cls(message)

    @classmethod
    def separator_not_found(cls, heading: str) -> MappingTableError:
        """Build an error for a table without its separator row."""
        message = f"mapping table under {heading!r} has no separator row"
        return cls(message)

    @classmethod
    def wrong_row_count(cls, heading: str, actual: int) -> MappingTableError:
        """Build an error for a table with the wrong number of data rows."""
        message = (
            f"mapping table under {heading!r} has {actual} data rows; "
            f"expected {EXPECTED_DATA_ROWS}"
        )
        return cls(message)

    @classmethod
    def table_not_found(cls, heading: str) -> MappingTableError:
        """Build an error for a missing mapping table."""
        message = f"mapping table not found under heading: {heading}"
        return cls(message)

    @classmethod
    def document_unreadable(
        cls, relative_path: Path, error: OSError
    ) -> MappingTableError:
        """Build an error for a document that could not be read."""
        message = f"could not read {relative_path}: {error}"
        return cls(message)


def normalise_table_row(row: str) -> str:
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
    """
    Collect lines from ``start`` until a same-or-higher-level heading appears.

    Parameters
    ----------
    lines : list[str]
        All document lines.
    start : int
        Index of the first line after the anchor heading.
    peer_pattern : re.Pattern[str]
        Compiled pattern that matches any heading at or above the anchor level.

    Returns
    -------
    list[str]
        Lines belonging to the section, excluding the terminating heading.
    """
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
        if match := heading_pattern.match(line):
            level = len(match.group("level"))
            peer_pattern = re.compile(rf"^#{{1,{level}}}\s+")
            return _collect_section_lines(lines, index + 1, peer_pattern)

    return None


def extract_mapping_rows(markdown: str, heading: str) -> list[str]:
    """
    Extract normalised GPUI mapping-table data rows from one document.

    Parameters
    ----------
    markdown : str
        Full document content.
    heading : str
        Section heading that anchors the relevant table.

    Returns
    -------
    list[str]
        The four ordered data rows, normalised for whitespace.

    Raises
    ------
    ValueError
        If the section or table cannot be found, or if the table has the wrong
        number of data rows.
    """
    section = find_section_after_heading(markdown, heading)
    if section is None:
        raise MappingTableError.heading_not_found(heading)

    for index, line in enumerate(section):
        if not line.startswith(TABLE_HEADER):
            continue
        separator_index = index + 1
        if separator_index >= len(section) or not section[separator_index].startswith(
            TABLE_SEPARATOR
        ):
            raise MappingTableError.separator_not_found(heading)

        rows: list[str] = []
        for row in section[separator_index + 1 :]:
            if not row.startswith("> |"):
                break
            rows.append(normalise_table_row(row))

        if len(rows) != EXPECTED_DATA_ROWS:
            raise MappingTableError.wrong_row_count(heading, len(rows))
        return rows

    raise MappingTableError.table_not_found(heading)


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
        Normalised data rows.
    """
    path = root / relative_path
    try:
        markdown = path.read_text(encoding="utf-8")
    except OSError as err:
        raise MappingTableError.document_unreadable(relative_path, err) from err
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
            violations.extend(
                [
                    f"row {index}:",
                    f"  {DESIGN_DOC}: {design_row}",
                    f"  {USERS_GUIDE}: {users_row}",
                ]
            )
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
