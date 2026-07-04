#!/usr/bin/env python3
"""
Validate that the duplicated `#[serial]`/nextest matrices stay in step.

The users' guide and the design document both explain how `#[serial]` behaves
under `cargo test` and cargo-nextest. This script catches doc-vs-doc drift
between those two copies by comparing their Markdown table data rows after
whitespace normalisation.

Usage
-----
python3 scripts/check_serial_nextest_matrix.py

Exit codes
----------
0
    The two runner-matrix data rows match after whitespace normalisation.
1
    A table is missing, malformed, or the two table bodies differ.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

DESIGN_DOC = Path("docs/rstest-bdd-design.md")
USERS_GUIDE = Path("docs/users-guide.md")

MATRIX_HEADING = "Test-runner parallelism and scenario state"
EXPECTED_DATA_ROWS = 2


class SerialNextestMatrixError(ValueError):
    """A `#[serial]`/nextest runner matrix is missing or malformed."""

    @classmethod
    def _for_heading(cls, template: str, heading: str) -> SerialNextestMatrixError:
        """Build an error from a heading-only message template."""
        return cls(template.format(heading=heading))

    @classmethod
    def heading_not_found(cls, heading: str) -> SerialNextestMatrixError:
        """
        Build an error for a missing anchor heading.

        Parameters
        ----------
        cls : type[SerialNextestMatrixError]
            The error type being constructed.
        heading : str
            The heading that was expected in the document.

        Returns
        -------
        SerialNextestMatrixError
            Error raised when the heading cannot be found.
        """
        return cls._for_heading("heading not found: {heading}", heading)

    @classmethod
    def separator_not_found(cls, heading: str) -> SerialNextestMatrixError:
        """
        Build an error for a table without its separator row.

        Parameters
        ----------
        cls : type[SerialNextestMatrixError]
            The error type being constructed.
        heading : str
            The heading that anchors the table.

        Returns
        -------
        SerialNextestMatrixError
            Error raised when the separator row is missing.
        """
        return cls._for_heading(
            "runner matrix under {heading!r} has no separator row", heading
        )

    @classmethod
    def wrong_row_count(cls, heading: str, actual: int) -> SerialNextestMatrixError:
        """
        Build an error for a table with the wrong number of data rows.

        Parameters
        ----------
        cls : type[SerialNextestMatrixError]
            The error type being constructed.
        heading : str
            The heading that anchors the table.
        actual : int
            The number of data rows that were found.

        Returns
        -------
        SerialNextestMatrixError
            Error raised when the table has the wrong number of rows.
        """
        message = (
            f"runner matrix under {heading!r} has {actual} data rows; "
            f"expected {EXPECTED_DATA_ROWS}"
        )
        return cls(message)

    @classmethod
    def table_not_found(cls, heading: str) -> SerialNextestMatrixError:
        """
        Build an error for a missing runner matrix.

        Parameters
        ----------
        cls : type[SerialNextestMatrixError]
            The error type being constructed.
        heading : str
            The heading that should contain the table.

        Returns
        -------
        SerialNextestMatrixError
            Error raised when the table cannot be found.
        """
        return cls._for_heading(
            "runner matrix not found under heading: {heading}", heading
        )

    @classmethod
    def document_unreadable(
        cls, relative_path: Path, error: OSError
    ) -> SerialNextestMatrixError:
        """
        Build an error for a document that could not be read.

        Parameters
        ----------
        cls : type[SerialNextestMatrixError]
            The error type being constructed.
        relative_path : Path
            The document path relative to the repository root.
        error : OSError
            The underlying read failure.

        Returns
        -------
        SerialNextestMatrixError
            Error raised when the document cannot be read.
        """
        message = f"could not read {relative_path}: {error}"
        return cls(message)


def normalise_table_row(row: str) -> str:
    """
    Collapse insignificant spacing in a Markdown table row.

    Parameters
    ----------
    row : str
        A raw Markdown table row.

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


def _is_table_row(line: str) -> bool:
    """Return whether *line* is a Markdown table row."""
    return line.startswith("|")


def _table_cells(line: str) -> list[str]:
    """Return trimmed cells from a Markdown table row."""
    return [cell.strip() for cell in line.strip().strip("|").split("|")]


def _is_runner_header(line: str) -> bool:
    """Return whether *line* is the runner matrix header row."""
    cells = _table_cells(line)
    return bool(cells) and cells[0] == "Runner"


def _is_separator_row(line: str) -> bool:
    """Return whether *line* is a Markdown table separator row."""
    cells = _table_cells(line)
    return bool(cells) and all(re.fullmatch(r":?-{3,}:?", cell) for cell in cells)


def _collect_table_rows(section: list[str], start: int) -> list[str]:
    """Return normalised data rows from *start*, stopping at the first non-row line."""
    rows: list[str] = []
    for row in section[start:]:
        if not _is_table_row(row):
            break
        rows.append(normalise_table_row(row))
    return rows


def _parse_table_at(section: list[str], header_index: int, heading: str) -> list[str]:
    """Validate the separator row and return the validated data rows."""
    separator_index = header_index + 1
    if separator_index >= len(section) or not _is_separator_row(
        section[separator_index]
    ):
        raise SerialNextestMatrixError.separator_not_found(heading)
    rows = _collect_table_rows(section, separator_index + 1)
    if len(rows) != EXPECTED_DATA_ROWS:
        raise SerialNextestMatrixError.wrong_row_count(heading, len(rows))
    return rows


def extract_matrix_rows(markdown: str, heading: str) -> list[str]:
    """
    Extract normalised runner-matrix data rows from one document.

    Parameters
    ----------
    markdown : str
        Full document content.
    heading : str
        Section heading that anchors the relevant table.

    Returns
    -------
    list[str]
        The two ordered data rows, normalised for whitespace.

    Raises
    ------
    ValueError
        If the section or table cannot be found, or if the table has the wrong
        number of data rows.
    """
    section = find_section_after_heading(markdown, heading)
    if section is None:
        raise SerialNextestMatrixError.heading_not_found(heading)

    for index, line in enumerate(section):
        if _is_runner_header(line):
            return _parse_table_at(section, index, heading)

    raise SerialNextestMatrixError.table_not_found(heading)


def read_matrix_rows(root: Path, relative_path: Path, heading: str) -> list[str]:
    """
    Read one document and extract its runner-matrix rows.

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
        raise SerialNextestMatrixError.document_unreadable(relative_path, err) from err
    return extract_matrix_rows(markdown, heading)


def check_matrix_tables(root: Path) -> list[str]:
    """
    Check that the users' guide and design runner matrices match.

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
        design_rows = read_matrix_rows(root, DESIGN_DOC, MATRIX_HEADING)
        users_rows = read_matrix_rows(root, USERS_GUIDE, MATRIX_HEADING)
    except ValueError as err:
        return [str(err)]

    if design_rows == users_rows:
        return []

    violations = ["`#[serial]`/nextest runner matrix data rows differ:"]
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
    """Check the duplicated `#[serial]`/nextest matrices and report violations."""
    root = Path(__file__).resolve().parents[1]
    violations = check_matrix_tables(root)
    for violation in violations:
        print(violation, file=sys.stderr)
    return 1 if violations else 0


if __name__ == "__main__":
    sys.exit(main())
