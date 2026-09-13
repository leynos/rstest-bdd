"""
Repository links in the users guide: their canonical form and generation.

``docs/users-guide.md`` is vendored into consumer projects, so its
cross-references to other documents in this repository use absolute GitHub URLs
rather than relative paths. This module records where those links point --
:data:`REPOSITORY_URL`, :data:`DEFAULT_BRANCH`, and :data:`DOCS_DIR` are the one
place either is written down -- and derives from them both the canonical form
of a definition and the reference block that generation writes.
``scripts/check_users_guide_links.py`` checks and regenerates the guide with it.

A definition that already carries :data:`BASE_URL` is canonical as written, so
a target document that has gone missing is reported rather than masked by a
rewrite. Any other definition is recognized by its shape -- a view URL on the
canonical host whose final path segment names a document in the documentation
tree -- which is what lets a definition written before the base URL moved still
be repaired: the target, not the base URL, is the identity.
"""

from pathlib import Path
from urllib.parse import urlparse

from markdown_references import REFERENCE_DEFINITION

# The guide's cross-references into this repository are described here and
# nowhere else: where the repository lives, which branch its documents are
# rendered from, and where the documentation tree starts. A repository move
# edits REPOSITORY_URL, a default-branch rename edits DEFAULT_BRANCH, and a
# documentation relocation edits DOCS_DIR; `make update-users-guide-links`
# then rewrites the guide's reference block from these values.
REPOSITORY_URL = "https://github.com/leynos/rstest-bdd"
DEFAULT_BRANCH = "main"
DOCS_DIR = "docs"
BASE_URL = f"{REPOSITORY_URL}/blob/{DEFAULT_BRANCH}/{DOCS_DIR}/"

# The guide the reference block lives in, named from the documentation tree so
# that relocating the tree moves the guide with it.
GUIDE = Path(DOCS_DIR) / "users-guide.md"

# Every link into this repository, document or not. A definition under this
# prefix stays this repository's business even when it cannot be rewritten -- an
# issue link, say -- so it is reported rather than passed off as a third-party
# link.
REPOSITORY_PREFIX = f"{REPOSITORY_URL}/"

# GitHub renders a file through a ``blob`` view URL. The segment before the
# document is the branch, which may itself contain ``/``, and the segment
# before that is the documentation root; neither is pinned here, so a link
# written before either of them changed is still recognized.
BLOB_SEGMENT = "/blob/"
CANONICAL_HOST = urlparse(REPOSITORY_URL).netloc


def document_target(root: Path, url: str) -> str | None:
    r"""
    Return the documentation target a repository link names, if it names one.

    The link is recognized by shape rather than by the base URL it was written
    against: a view URL on the canonical host whose final path segment is a
    document in the documentation tree. That is what lets a definition written
    before a branch rename, a repository move, or a documentation relocation
    still be regenerated. Documents below the documentation root fall outside
    this shape and are reported rather than silently rewritten.

    Parameters
    ----------
    root : Path
        The repository root directory.
    url : str
        The reference URL under test.

    Returns
    -------
    str | None
        The documentation-relative target with any fragment, or ``None`` when
        the link names no document in the documentation tree.

    Examples
    --------
    >>> import tempfile
    >>> root = Path(tempfile.mkdtemp())
    >>> (root / "docs").mkdir()
    >>> _ = (root / "docs" / "other.md").write_text("# Other\n", encoding="utf-8")
    >>> document_target(root, f"{BASE_URL}other.md")
    'other.md'
    >>> document_target(root, f"{REPOSITORY_URL}/issues/537") is None
    True
    """
    parsed = urlparse(url)
    if parsed.netloc != CANONICAL_HOST or BLOB_SEGMENT not in parsed.path:
        return None
    name = parsed.path.rpartition("/")[2]
    if not (root / DOCS_DIR / name).is_file():
        return None
    return f"{name}#{parsed.fragment}" if parsed.fragment else name


def canonical_link(root: Path, url: str) -> str | None:
    r"""
    Return the canonical form of a repository document link.

    A link that already carries :data:`BASE_URL` is canonical as written --
    including one whose target document is missing, which the caller reports
    separately. Any other link is rewritten from the target it names, so a
    definition written against an earlier base URL is repaired rather than
    rejected.

    Parameters
    ----------
    root : Path
        The repository root directory.
    url : str
        The reference URL under test.

    Returns
    -------
    str | None
        The canonical URL, or ``None`` when the link names no document in the
        documentation tree.

    Examples
    --------
    >>> import tempfile
    >>> root = Path(tempfile.mkdtemp())
    >>> (root / "docs").mkdir()
    >>> _ = (root / "docs" / "other.md").write_text("# Other\n", encoding="utf-8")
    >>> canonical_link(root, f"{REPOSITORY_URL}/blob/old/docs/other.md")
    'https://github.com/leynos/rstest-bdd/blob/main/docs/other.md'
    >>> canonical_link(root, f"{REPOSITORY_URL}/issues/537") is None
    True
    """
    if url.startswith(BASE_URL):
        return url
    target = document_target(root, url)
    return None if target is None else f"{BASE_URL}{target}"


def generate_guide(root: Path, markdown: str) -> str:
    r"""
    Return *markdown* with every repository document link made canonical.

    Only reference definition lines change, and only their URL: labels, order,
    spacing, and line endings are preserved, so the guide stays plain Markdown
    for consumers and a rewrite cannot disturb prose. A definition whose target
    cannot be recovered is left for the caller to report.

    Parameters
    ----------
    root : Path
        The repository root directory.
    markdown : str
        The guide content.

    Returns
    -------
    str
        The guide content with canonical repository document links.

    Examples
    --------
    >>> import tempfile
    >>> root = Path(tempfile.mkdtemp())
    >>> (root / "docs").mkdir()
    >>> _ = (root / "docs" / "other.md").write_text("# Other\n", encoding="utf-8")
    >>> generate_guide(root, f"[other]: {REPOSITORY_URL}/blob/old/docs/other.md\n")
    '[other]: https://github.com/leynos/rstest-bdd/blob/main/docs/other.md\n'
    """
    lines: list[str] = []
    for line in markdown.splitlines(keepends=True):
        match = REFERENCE_DEFINITION.match(line)
        if match is None:
            lines.append(line)
            continue
        canonical = canonical_link(root, match.group("url"))
        if canonical is None or canonical == match.group("url"):
            lines.append(line)
            continue
        lines.append(
            f"[{match.group('label')}]:{match.group('separator')}{canonical}"
            f"{match.group('trailing')}"
        )
    return "".join(lines)


def count_changed_lines(committed: str, generated: str) -> int:
    r"""
    Count the lines that differ between the committed and generated guide.

    Generation only rewrites lines that already exist, so the two texts hold
    the same number of lines; the pairing is strict to keep that a checked
    invariant rather than an assumption.

    Parameters
    ----------
    committed : str
        The guide content on disk.
    generated : str
        The content generation would write.

    Returns
    -------
    int
        The number of lines that differ.

    Examples
    --------
    >>> count_changed_lines("one\ntwo\nthree\n", "one\nTWO\nthree\n")
    1
    """
    pairs = zip(committed.splitlines(), generated.splitlines(), strict=True)
    return sum(
        1
        for committed_line, generated_line in pairs
        if committed_line != generated_line
    )


def is_repository_link(root: Path, url: str) -> bool:
    """
    Return whether *url* is one of this repository's document links.

    Definitions under the repository prefix are checked even when they cannot
    be rewritten, so a link to a document that has been renamed is still
    reported. A definition the generator would rewrite counts too, which is
    what keeps a reference block written before the base URL moved inside the
    check rather than being dismissed as somebody else's link.

    Parameters
    ----------
    root : Path
        The repository root directory.
    url : str
        The reference URL under test.

    Returns
    -------
    bool
        True when the link belongs to this repository's reference block.

    Examples
    --------
    >>> import tempfile
    >>> root = Path(tempfile.mkdtemp())
    >>> is_repository_link(root, f"{REPOSITORY_URL}/issues/537")
    True
    >>> is_repository_link(root, "https://docs.rs/rstest-bdd")
    False
    """
    return url.startswith(REPOSITORY_PREFIX) or canonical_link(root, url) is not None
