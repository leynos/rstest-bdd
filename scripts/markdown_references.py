"""
Markdown document structure: heading anchors and reference definitions.

The users guide is validated and regenerated from its reference definition
lines, and its fragments are checked against the anchors GitHub derives from
headings. Both are Markdown mechanics rather than guide policy, so they live
here; ``scripts/users_guide_links.py`` decides what the links should say.
"""

import re

HEADING = re.compile(r"^#{1,6}\s+(?P<text>.*)$")
REFERENCE_DEFINITION = re.compile(
    r"^\[(?P<label>[^\]]+)\]:(?P<separator>\s*)(?P<url>\S+)(?P<trailing>\s*)$"
)


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
