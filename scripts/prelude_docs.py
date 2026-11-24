#!/usr/bin/env python3
"""
Find and read the documentation that describes the prelude.

The drift check compares Rust source with three documents: the users' guide's
import section, whose table and examples must agree with the prelude, and the
READMEs, whose braced imports must name real crate-root exports. This module is
the document side of that comparison. It locates a section by its GitHub
anchor, reads Markdown tables and fenced blocks, and reads a repository file
without raising when it is absent.

`scripts/prelude_drift.py` turns these pieces into violations, and
`scripts/check_prelude_exports.py` runs the whole check.
"""

from pathlib import Path

from markdown_references import FENCE, HEADING, github_heading_anchor

USERS_GUIDE = Path("docs/users-guide.md")
READMES = (
    Path("README.md"),
    Path("crates/rstest-bdd/README.md"),
    Path("crates/rstest-bdd-macros/README.md"),
)

SECTION_ANCHOR = "importing-rstest-bdd"

# The section opens and closes at heading levels up to this depth, so a
# subsection written inside it does not end the region.
SECTION_HEADING_LEVEL = 2


def read_text(root: Path, relative_path: Path) -> str | None:
    """
    Read one repository file, reporting the failure instead of raising.

    Parameters
    ----------
    root : Path
        Repository root directory.
    relative_path : Path
        File path relative to ``root``.

    Returns
    -------
    str | None
        The file's text, or ``None`` when it could not be read.
    """
    try:
        return (root / relative_path).read_text(encoding="utf-8")
    except OSError:
        return None


def heading_level(line: str) -> int:
    """
    Return a Markdown heading line's level, which is its leading hash count.

    Parameters
    ----------
    line : str
        The line to measure. It must be a heading.

    Returns
    -------
    int
        The heading level, from 1 for ``# `` upward.
    """
    return len(line) - len(line.lstrip("#"))


def opens_section(line: str, anchor: str) -> bool:
    """
    Return whether a line is the heading that opens the requested section.

    Parameters
    ----------
    line : str
        The line to inspect.
    anchor : str
        GitHub anchor fragment of the heading that opens the section.

    Returns
    -------
    bool
        Whether the line is a level-two heading with that anchor.
    """
    heading = HEADING.match(line)
    if heading is None:
        return False
    return (
        heading_level(line) == SECTION_HEADING_LEVEL
        and github_heading_anchor(heading.group("text").strip()) == anchor
    )


def prose_headings(lines: list[str]) -> list[tuple[int, int]]:
    """
    Return every heading that sets document structure, with its line index.

    A heading inside a fenced block is sample content rather than structure, so
    it is skipped; otherwise an example that shows a heading would truncate the
    section that contains it.

    Parameters
    ----------
    lines : list[str]
        Lines of one document.

    Returns
    -------
    list[tuple[int, int]]
        ``(line index, heading level)`` per structural heading, in order.
    """
    in_fence = False
    headings: list[tuple[int, int]] = []
    for index, line in enumerate(lines):
        if FENCE.match(line) is not None:
            in_fence = not in_fence
            continue
        if in_fence or HEADING.match(line) is None:
            continue
        headings.append((index, heading_level(line)))
    return headings


def section_bounds(lines: list[str], anchor: str) -> tuple[int, int] | None:
    """
    Return the half-open line range of a section's body, heading excluded.

    Parameters
    ----------
    lines : list[str]
        Lines of one document.
    anchor : str
        GitHub anchor fragment of the heading that opens the section.

    Returns
    -------
    tuple[int, int] | None
        The body's start and end line indexes, or ``None`` when the anchor is
        absent. A section that runs to the end of the document ends at its
        length.
    """
    headings = prose_headings(lines)
    starts = [index + 1 for index, _ in headings if opens_section(lines[index], anchor)]
    if not starts:
        return None
    start = starts[0]
    closes = [
        index
        for index, level in headings
        if index >= start and level <= SECTION_HEADING_LEVEL
    ]
    return start, closes[0] if closes else len(lines)


def find_section(markdown: str, anchor: str) -> list[str] | None:
    """
    Return the lines of the level-two section whose heading anchor matches.

    Parameters
    ----------
    markdown : str
        Full document content.
    anchor : str
        GitHub anchor fragment of the heading that opens the section.

    Returns
    -------
    list[str] | None
        The section's lines without its heading, or ``None`` when absent.
    """
    lines = markdown.splitlines()
    bounds = section_bounds(lines, anchor)
    return None if bounds is None else lines[bounds[0] : bounds[1]]


def table_rows(section: list[str]) -> list[list[str]]:
    """
    Return the cells of the first Markdown table in a section.

    Parameters
    ----------
    section : list[str]
        Lines of one document section.

    Returns
    -------
    list[list[str]]
        One list of cells per row, including the header and separator.
    """
    rows: list[list[str]] = []
    for line in section:
        stripped = line.strip()
        if not stripped.startswith("|"):
            if rows:
                break
            continue
        rows.append([
            cell.strip().strip("`") for cell in stripped.strip("|").split("|")
        ])
    return rows


def code_block_bodies(section: list[str]) -> list[str]:
    """
    Return the bodies of every fenced code block in a section.

    Parameters
    ----------
    section : list[str]
        Lines of one document section.

    Returns
    -------
    list[str]
        One body per fenced block, in document order.
    """
    bodies: list[str] = []
    current: list[str] | None = None
    for line in section:
        if FENCE.match(line) is not None:
            if current is None:
                current = []
            else:
                bodies.append("\n".join(current))
                current = None
            continue
        if current is not None:
            current.append(line)
    return bodies
