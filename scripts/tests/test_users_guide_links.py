"""Unit tests for the users-guide repository link module."""

from pathlib import Path

import pytest
from hypothesis import given
from hypothesis import strategies as st
from users_guide_links import (
    BASE_URL,
    DOCS_DIR,
    REPOSITORY_URL,
    canonical_link,
    count_changed_lines,
    document_target,
    generate_guide,
    is_repository_link,
)

#: The repository the properties below generate against. They take no fixture
#: arguments -- Hypothesis evaluates test annotations at run time, so a
#: ``Path`` parameter would need :mod:`pathlib` outside a type-checking block
#: anyway -- and the real documentation tree is the one that matters.
REPO_ROOT = Path(__file__).resolve().parents[2]

#: A URL written against a branch the repository no longer uses. Its target,
#: not its base, is the identity recovery depends on.
STALE_BRANCH_URL = f"{REPOSITORY_URL}/blob/master/{DOCS_DIR}/target.md"

#: Lines a guide may hold: prose, definitions this repository owns in either
#: the canonical or a stale form, definitions whose target has gone, and
#: third-party definitions. Each carries its own line ending. The document
#: named is one the repository really holds, so a property that generates
#: against the real tree can rewrite it.
GUIDE_LINE_POOL: tuple[str, ...] = (
    "Prose with an [inline](https://example.com) link.\n",
    "\n",
    "[docs-rs]: https://docs.rs/rstest-bdd/latest/\n",
    f"[canonical]: {BASE_URL}developers-guide.md\n",
    f"[canonical-fragment]: {BASE_URL}developers-guide.md#developer-guide\n",
    f"[stale]: {REPOSITORY_URL}/blob/master/{DOCS_DIR}/developers-guide.md\n",
    f"[missing]: {BASE_URL}gone.md\n",
    f"[unrecoverable]: {BASE_URL}nested/gone.md\n",
    f"[spaced]:  {REPOSITORY_URL}/blob/master/{DOCS_DIR}/developers-guide.md  \n",
)


@pytest.fixture
def repo(tmp_path: Path) -> Path:
    """Create a repository root holding one documentation file."""
    docs = tmp_path / DOCS_DIR
    docs.mkdir()
    (docs / "target.md").write_text("# Target\n\n## Section A\n", encoding="utf-8")
    return tmp_path


class TestDocumentTarget:
    """Tests for :func:`users_guide_links.document_target`."""

    def test_recovers_target_from_canonical_link(self, repo: Path) -> None:
        """A canonical link should name its document."""
        target = document_target(repo, f"{BASE_URL}target.md")
        assert target == "target.md", f"expected target.md, got {target!r}"

    def test_recovers_target_and_fragment(self, repo: Path) -> None:
        """A fragment should travel with the recovered target."""
        target = document_target(repo, f"{BASE_URL}target.md#section-a")
        assert target == "target.md#section-a", f"unexpected target: {target!r}"

    def test_recovers_target_written_against_a_stale_branch(self, repo: Path) -> None:
        """A branch rename must not hide the target the link names."""
        target = document_target(repo, STALE_BRANCH_URL)
        assert target == "target.md", f"stale branch should recover, got {target!r}"

    def test_recovers_target_from_a_relocated_repository(self, repo: Path) -> None:
        """A relocation must not hide the target the link names either."""
        url = f"https://github.com/df12-org/rstest-bdd/blob/main/{DOCS_DIR}/target.md"
        target = document_target(repo, url)
        assert target == "target.md", f"relocation should recover, got {target!r}"

    def test_rejects_links_on_another_host(self, repo: Path) -> None:
        """A host that is not the canonical one is somebody else's link."""
        url = f"https://example.com/blob/main/{DOCS_DIR}/target.md"
        target = document_target(repo, url)
        assert target is None, f"foreign host should not be recognized, got {target!r}"

    def test_rejects_repository_urls_that_are_not_view_urls(self, repo: Path) -> None:
        """An issue URL names no document."""
        target = document_target(repo, f"{REPOSITORY_URL}/issues/537")
        assert target is None, f"issue URL should name no document, got {target!r}"

    def test_rejects_documents_that_do_not_exist(self, repo: Path) -> None:
        """A document outside the documentation tree is not recoverable."""
        target = document_target(repo, f"{BASE_URL}gone.md")
        assert target is None, f"absent document should not recover, got {target!r}"

    def test_rejects_nested_documents(self, repo: Path) -> None:
        """Documents below the documentation root are outside the shape.

        The recovery rule reads the final path segment as the document, which
        holds because the documentation tree is flat; a nested path therefore
        only recovers when its basename names a document at the root.
        """
        target = document_target(repo, f"{BASE_URL}nested/gone.md")
        assert target is None, f"nested path should not recover, got {target!r}"


class TestCanonicalLink:
    """Tests for :func:`users_guide_links.canonical_link`."""

    def test_leaves_a_canonical_link_unchanged(self, repo: Path) -> None:
        """A canonical link is already in its canonical form."""
        url = f"{BASE_URL}target.md#section-a"
        assert canonical_link(repo, url) == url, "canonical link should be unchanged"

    def test_keeps_a_canonical_link_to_a_missing_document(self, repo: Path) -> None:
        """A missing target is reported rather than masked by a rewrite.

        Returning ``None`` here would turn "this document has gone" into a
        generic "not a repository link" and let the drift through.
        """
        url = f"{BASE_URL}gone.md"
        assert canonical_link(repo, url) == url, (
            "a canonical link must survive canonicalization so the checker"
            " can report its missing document"
        )

    def test_rewrites_a_link_from_a_stale_branch(self, repo: Path) -> None:
        """A branch rename is repaired by regenerating the link."""
        canonical = canonical_link(repo, STALE_BRANCH_URL)
        assert canonical == f"{BASE_URL}target.md", f"unexpected canonical: {canonical}"

    def test_rewrites_a_link_from_a_relocated_repository(self, repo: Path) -> None:
        """A repository move is repaired by regenerating the link."""
        url = f"https://github.com/df12-org/rstest-bdd/blob/main/{DOCS_DIR}/target.md"
        canonical = canonical_link(repo, url)
        assert canonical == f"{BASE_URL}target.md", f"unexpected canonical: {canonical}"

    def test_rejects_foreign_links(self, repo: Path) -> None:
        """Links to other projects are left alone."""
        url = f"https://example.com/blob/main/{DOCS_DIR}/target.md"
        canonical = canonical_link(repo, url)
        assert canonical is None, (
            f"foreign link should not canonicalize, got {canonical}"
        )

    def test_rejects_repository_links_that_name_no_document(self, repo: Path) -> None:
        """A repository URL outside the shape is not canonicalizable."""
        canonical = canonical_link(repo, f"{REPOSITORY_URL}/issues/537")
        assert canonical is None, f"issue URL should not canonicalize, got {canonical}"


class TestGenerateGuide:
    """Tests for :func:`users_guide_links.generate_guide`."""

    def test_rewrites_the_url_of_a_stale_definition(self, repo: Path) -> None:
        """A definition written against an older branch is repaired."""
        markdown = f"[adr-001]: {STALE_BRANCH_URL}\n"
        generated = generate_guide(repo, markdown)
        assert generated == f"[adr-001]: {BASE_URL}target.md\n", (
            f"stale definition should be rewritten, got {generated!r}"
        )

    def test_preserves_label_spacing_and_line_endings(self, repo: Path) -> None:
        """Only the URL changes; the rest of the line survives verbatim."""
        markdown = f"[spaced]:  {STALE_BRANCH_URL}  \r\n"
        generated = generate_guide(repo, markdown)
        assert generated == f"[spaced]:  {BASE_URL}target.md  \r\n", (
            f"spacing and line endings should be preserved, got {generated!r}"
        )

    def test_preserves_a_missing_trailing_newline(self, repo: Path) -> None:
        """A file that does not end in a newline still does not."""
        generated = generate_guide(repo, f"[a]: {STALE_BRANCH_URL}")
        assert generated == f"[a]: {BASE_URL}target.md", (
            f"the final newline should not appear, got {generated!r}"
        )

    def test_leaves_prose_and_foreign_links_untouched(self, repo: Path) -> None:
        """Generation only touches this repository's document links."""
        markdown = (
            "Prose with an [inline](https://example.com) link.\n"
            "[docs-rs]: https://docs.rs/rstest-bdd/latest/\n"
            f"[hosted-elsewhere]: https://example.com/blob/main/{DOCS_DIR}/target.md\n"
        )
        generated = generate_guide(repo, markdown)
        assert generated == markdown, f"nothing should change, got {generated!r}"

    def test_leaves_an_unrecoverable_definition_for_the_checker(
        self, repo: Path
    ) -> None:
        """A definition the generator cannot repair is left to be reported."""
        markdown = f"[nested]: {BASE_URL}nested/gone.md\n"
        generated = generate_guide(repo, markdown)
        assert generated == markdown, (
            f"unrecoverable line should survive, got {generated!r}"
        )

    def test_rewrites_every_stale_definition(self, repo: Path) -> None:
        """Each stale definition in the block is rewritten."""
        markdown = (
            f"[one]: {STALE_BRANCH_URL}\n"
            f"[two]: {BASE_URL}target.md#section-a\n"
            f"[three]: {STALE_BRANCH_URL}#section-a\n"
        )
        generated = generate_guide(repo, markdown)
        assert generated == (
            f"[one]: {BASE_URL}target.md\n"
            f"[two]: {BASE_URL}target.md#section-a\n"
            f"[three]: {BASE_URL}target.md#section-a\n"
        ), f"every stale definition should be rewritten, got {generated!r}"


class TestCountChangedLines:
    """Tests for :func:`users_guide_links.count_changed_lines`."""

    def test_identical_content_changes_nothing(self) -> None:
        """Two identical guides differ in no lines."""
        text = "one\ntwo\n"
        assert count_changed_lines(text, text) == 0, "identical text should not differ"

    def test_counts_only_the_differing_lines(self) -> None:
        """Lines that match are not counted."""
        committed = f"same\n[a]: {STALE_BRANCH_URL}\nsame\n"
        generated = f"same\n[a]: {BASE_URL}target.md\nsame\n"
        changed = count_changed_lines(committed, generated)
        assert changed == 1, f"expected one changed line, got {changed}"

    def test_rejects_texts_of_differing_length(self) -> None:
        """Generation rewrites lines rather than adding or removing them."""
        with pytest.raises(ValueError, match="longer than"):
            count_changed_lines("one\n", "one\ntwo\n")


class TestIsRepositoryLink:
    """Tests for :func:`users_guide_links.is_repository_link`."""

    def test_accepts_canonical_links(self, repo: Path) -> None:
        """A canonical definition belongs to the reference block."""
        assert is_repository_link(repo, f"{BASE_URL}target.md"), (
            "canonical link should count as a repository link"
        )

    def test_accepts_links_written_against_a_stale_branch(self, repo: Path) -> None:
        """A pre-move definition stays inside the check rather than being skipped."""
        assert is_repository_link(repo, STALE_BRANCH_URL), (
            "a recoverable pre-move link should still be checked"
        )

    def test_accepts_repository_urls_that_name_no_document(self, repo: Path) -> None:
        """A repository URL is reported even when it cannot be rewritten."""
        assert is_repository_link(repo, f"{REPOSITORY_URL}/issues/537"), (
            "repository URLs should be checked whatever they name"
        )

    def test_rejects_third_party_links(self, repo: Path) -> None:
        """Links to other projects and hosts are ignored."""
        urls = (
            "https://docs.rs/rstest-bdd/latest/",
            f"https://example.com/blob/main/{DOCS_DIR}/target.md",
        )
        for url in urls:
            assert not is_repository_link(repo, url), f"{url} is not ours to check"


class TestGenerateGuideProperties:
    """Property tests for :func:`users_guide_links.generate_guide`."""

    @given(lines=st.lists(st.sampled_from(GUIDE_LINE_POOL), max_size=20))
    def test_generation_is_idempotent(self, lines: list[str]) -> None:
        """Regenerating a generated block must not change it again."""
        markdown = "".join(lines)
        once = generate_guide(REPO_ROOT, markdown)
        twice = generate_guide(REPO_ROOT, once)
        assert twice == once, f"generation should be idempotent: {once!r} -> {twice!r}"

    @given(lines=st.lists(st.sampled_from(GUIDE_LINE_POOL), max_size=20))
    def test_generation_preserves_line_count(self, lines: list[str]) -> None:
        """Generation rewrites lines, so the line count is a fixed point.

        The checker pairs committed and generated lines strictly, so this is
        the invariant that makes that pairing safe.
        """
        markdown = "".join(lines)
        generated = generate_guide(REPO_ROOT, markdown)
        assert len(generated.splitlines()) == len(markdown.splitlines()), (
            f"line count changed: {markdown!r} -> {generated!r}"
        )
