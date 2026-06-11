#!/usr/bin/env python3
"""
Validate the absolute repository links in ``docs/users-guide.md``.

The users guide is vendored into consumer projects, so its cross-references
to other documents in this repository use absolute GitHub URLs rather than
relative paths. ``BASE_URL`` below is the single place that records the
canonical prefix; this script fails when a reference definition drifts from
that prefix, points at a document that no longer exists, or carries a
fragment that no longer matches a heading in the target document. A branch
rename or doc relocation therefore surfaces as a lint failure with one
constant to update. It is invoked automatically by the ``make lint`` target.

Usage
-----
python3 scripts/check_users_guide_links.py

Exit codes
----------
0
    Every repository link uses the canonical base URL, resolves to an
    existing document, and any fragment matches a heading anchor.
1
    Violations found, or the guide itself could not be read.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

GUIDE = Path("docs/users-guide.md")
# Canonical prefix for cross-references into this repository. Update this
# single constant if the repository moves, the default branch is renamed, or
# the documents relocate.
BASE_URL = "https://github.com/leynos/rstest-bdd/blob/main/docs/"
REPO_URL_PREFIX = "https://github.com/leynos/rstest-bdd/"

REFERENCE_DEFINITION = re.compile(r"^\[(?P<label>[^\]]+)\]:\s*(?P<url>\S+)\s*$")
HEADING = re.compile(r"^#{1,6}\s+(?P<text>.*)$")


def github_heading_anchor(heading: str) -> str:
    """
    Derive the GitHub anchor fragment for a Markdown heading.

    GitHub lowercases the heading, strips formatting characters and
    punctuation, and replaces spaces with hyphens.

    Parameters
    ----------
    heading : str
        The heading text without the leading ``#`` markers.

    Returns
    -------
    str
        The anchor fragment GitHub generates for the heading.
    """
    text = heading.strip().lower().replace("`", "")
    text = re.sub(r"[^\w\- ]", "", text)
    return text.replace(" ", "-")


def heading_anchors(markdown: str) -> set[str]:
    """
    Collect the GitHub anchor fragments for every heading in a document.

    Lines inside fenced code blocks are ignored so that ``#`` comments in
    code samples are not mistaken for headings.

    Parameters
    ----------
    markdown : str
        The document content.

    Returns
    -------
    set[str]
        The anchor fragments GitHub generates for the document's headings.
    """
    anchors: set[str] = set()
    in_code_fence = False
    for line in markdown.splitlines():
        if line.lstrip().startswith("```"):
            in_code_fence = not in_code_fence
            continue
        if not in_code_fence and (match := HEADING.match(line)):
            anchors.add(github_heading_anchor(match.group("text")))
    return anchors


def reference_definitions(markdown: str) -> list[tuple[str, str]]:
    """
    Extract ``[label]: url`` reference definitions from guide content.

    Parameters
    ----------
    markdown : str
        The guide content.

    Returns
    -------
    list[tuple[str, str]]
        ``(label, url)`` pairs in document order.
    """
    return [
        (match.group("label"), match.group("url"))
        for line in markdown.splitlines()
        if (match := REFERENCE_DEFINITION.match(line))
    ]


def check_repo_link(root: Path, label: str, url: str) -> list[str]:
    """
    Validate one repository reference definition.

    Parameters
    ----------
    root : Path
        The repository root directory.
    label : str
        The reference label, used in violation messages.
    url : str
        The reference URL.

    Returns
    -------
    list[str]
        Human-readable violations; empty when the reference is valid.
    """
    if not url.startswith(BASE_URL):
        return [f"[{label}] does not use the canonical base URL {BASE_URL}: {url}"]

    target, _, fragment = url.removeprefix(BASE_URL).partition("#")
    document = root / "docs" / target
    try:
        content = document.read_text(encoding="utf-8")
    except FileNotFoundError:
        return [f"[{label}] points at a missing document: docs/{target}"]
    except OSError as err:
        return [f"[{label}] points at an unreadable document: docs/{target} ({err})"]

    if fragment and fragment not in heading_anchors(content):
        return [f"[{label}] fragment #{fragment} matches no heading in docs/{target}"]

    return []


def check_guide(root: Path) -> list[str]:
    """
    Check every repository reference link in the guide.

    Parameters
    ----------
    root : Path
        The repository root directory.

    Returns
    -------
    list[str]
        Human-readable violations; empty when every link is valid.
    """
    guide = root / GUIDE
    try:
        markdown = guide.read_text(encoding="utf-8")
    except OSError as err:
        return [f"could not read {GUIDE}: {err}"]

    violations: list[str] = []
    repo_links = 0
    for label, url in reference_definitions(markdown):
        if not url.startswith(REPO_URL_PREFIX):
            continue
        repo_links += 1
        violations.extend(check_repo_link(root, label, url))

    if repo_links == 0:
        violations.append(
            f"no repository reference links found in {GUIDE}; "
            "the reference block may have been removed or reformatted"
        )

    return violations


def main() -> int:
    """Check the guide's repository links and report any violations."""
    root = Path(__file__).resolve().parents[1]
    violations = check_guide(root)
    for violation in violations:
        print(violation, file=sys.stderr)
    return 1 if violations else 0


if __name__ == "__main__":
    sys.exit(main())
