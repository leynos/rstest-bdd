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
FENCE = re.compile(r"^ {0,3}(?P<fence>(?P<char>[`~])(?P=char){2,})(?P<info>.*)$")

# GitHub anchors a heading by the text it displays rather than by its source,
# so inline markup contributes its content and loses its delimiters. Emphasis
# delimiters are dropped only in pairs: an intraword underscore, such as the
# one in ``file_serial``, is literal text and survives into the anchor.
LINK = re.compile(r"!?\[(?P<label>[^\]]*)\]\([^)]*\)")
EMPHASIS = re.compile(
    r"(?<!\w)(?P<delim>[*_]{1,3})(?=\S)(?P<text>.+?)(?<=\S)(?P=delim)(?!\w)"
)


def _heading_text(heading: str) -> str:
    """
    Return the text a Markdown heading displays.

    Anchors come from rendered text, so inline markup is reduced to what it
    shows: a code span keeps its content without the backticks, a link or image
    keeps its label without the destination, and an emphasis pair keeps the
    text it encloses without the delimiters.

    Parameters
    ----------
    heading : str
        The heading text without the leading ``#`` markers.

    Returns
    -------
    str
        The text the heading displays.
    """
    without_code = heading.replace("`", "")
    return EMPHASIS.sub(r"\g<text>", LINK.sub(r"\g<label>", without_code))


def github_heading_anchor(heading: str) -> str:
    r"""
    Derive the GitHub anchor fragment for a Markdown heading.

    GitHub lowercases the heading's displayed text, strips punctuation, and
    replaces spaces with hyphens. Formatting is not text, so emphasis
    delimiters, code-span backticks, and link destinations are gone before the
    text is slugged; an underscore that is not an emphasis delimiter, such as
    the one in ``file_serial``, is text and survives.

    Parameters
    ----------
    heading : str
        The heading text without the leading ``#`` markers.

    Returns
    -------
    str
        The anchor fragment GitHub generates for the heading.

    Examples
    --------
    >>> github_heading_anchor("_Helpful_")
    'helpful'
    >>> github_heading_anchor("foo_bar")
    'foo_bar'
    """
    text = _heading_text(heading).strip().lower()
    text = re.sub(r"[^\w\- ]", "", text)
    return text.replace(" ", "-")


def _fence_state(
    fence: tuple[str, int] | None, delimiter: str, info: str
) -> tuple[str, int] | None:
    """
    Return the code fence left open by a delimiter line.

    A fence opens on a run of three or more backticks or tildes and closes on a
    run of the same character that is at least as long and carries no info
    string; anything else inside a fence is content. Tracking the character and
    the length together is what keeps a ``~~~`` block from being read as
    headings, and what stops a short run from closing a longer fence that
    encloses it.

    Parameters
    ----------
    fence : tuple[str, int] | None
        The open fence's character and run length, or ``None`` when no fence
        is open.
    delimiter : str
        The run of fence characters that starts this line.
    info : str
        Whatever follows the run on this line.

    Returns
    -------
    tuple[str, int] | None
        The fence still open once this line has been read.
    """
    if fence is None:
        return (delimiter[0], len(delimiter))
    character, length = fence
    if delimiter[0] != character or len(delimiter) < length:
        return fence
    if info.strip():
        return fence
    return None


def _unique_anchor(anchor: str, occurrences: dict[str, int]) -> str:
    """
    Return *anchor*, numerically suffixed when an earlier heading claimed it.

    GitHub keeps the first heading's slug and appends ``-1``, ``-2``, and so on
    to later headings that slug identically, testing each candidate against
    every slug already handed out. The counter is keyed on the base slug, so a
    heading whose own slug was generated as a suffix is suffixed again rather
    than taking the next free number.

    Parameters
    ----------
    anchor : str
        The base slug of the heading being anchored.
    occurrences : dict[str, int]
        The counters and claimed slugs of the headings read so far, updated in
        place.

    Returns
    -------
    str
        The unique anchor for this heading.
    """
    base = anchor
    while anchor in occurrences:
        occurrences[base] = occurrences.get(base, 0) + 1
        anchor = f"{base}-{occurrences[base]}"
    occurrences[anchor] = 0
    return anchor


def heading_anchors(markdown: str) -> set[str]:
    r"""
    Collect the GitHub anchor fragments for every heading in a document.

    Lines inside fenced code blocks are ignored so that ``#`` comments in
    code samples are not mistaken for headings. Both backtick and tilde fences
    are recognized, indented by up to three spaces. Headings that slug
    identically are anchored as GitHub anchors them: the first keeps the slug
    and the rest take incrementing numeric suffixes.

    Parameters
    ----------
    markdown : str
        The document content.

    Returns
    -------
    set[str]
        The anchor fragments GitHub generates for the document's headings.

    Examples
    --------
    >>> sorted(heading_anchors("```\n# Skipped\n```\n# Kept\n"))
    ['kept']
    >>> sorted(heading_anchors("~~~\n# Skipped\n~~~\n# Kept\n"))
    ['kept']
    >>> sorted(heading_anchors("# Setup\n\n## Setup\n"))
    ['setup', 'setup-1']
    """
    anchors: set[str] = set()
    occurrences: dict[str, int] = {}
    fence: tuple[str, int] | None = None
    for line in markdown.splitlines():
        if (fence_match := FENCE.match(line)) is not None:
            fence = _fence_state(
                fence, fence_match.group("fence"), fence_match.group("info")
            )
            continue
        if fence is not None:
            continue
        if (heading := HEADING.match(line)) is not None:
            anchor = _unique_anchor(
                github_heading_anchor(heading.group("text")), occurrences
            )
            anchors.add(anchor)
    return anchors


def reference_definitions(markdown: str) -> list[tuple[str, str]]:
    r"""
    Extract ``[label]: url`` reference definitions from guide content.

    Parameters
    ----------
    markdown : str
        The guide content.

    Returns
    -------
    list[tuple[str, str]]
        ``(label, url)`` pairs in document order.

    Examples
    --------
    >>> reference_definitions("[b]: ./b.md\nSee [a](a.md).\n[a]: ./a.md\n")
    [('b', './b.md'), ('a', './a.md')]
    """
    return [
        (match.group("label"), match.group("url"))
        for line in markdown.splitlines()
        if (match := REFERENCE_DEFINITION.match(line))
    ]
