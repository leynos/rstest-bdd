"""Unit tests for the users-guide link checker."""

from __future__ import annotations

import re
import typing as typ

import pytest
from check_users_guide_links import (
    BASE_URL,
    GUIDE,
    check_guide,
    check_repo_link,
    github_heading_anchor,
    heading_anchors,
    reference_definitions,
)
from hypothesis import given
from hypothesis import strategies as st

if typ.TYPE_CHECKING:
    from pathlib import Path

# Characters at which str.splitlines() breaks a line. Heading strategies
# exclude them so a generated heading stays on a single Markdown line.
LINE_BREAKS: str = "\n\r\x0b\x0c\x1c\x1d\x1e\x85\u2028\u2029"

single_line_text: st.SearchStrategy[str] = st.text(
    alphabet=st.characters(exclude_characters=LINE_BREAKS),
    max_size=80,
)


class TestGithubHeadingAnchor:
    """Tests for :func:`check_users_guide_links.github_heading_anchor`."""

    @pytest.mark.parametrize(
        ("heading", "anchor"),
        [
            ("Plain heading", "plain-heading"),
            (
                "Section 1.2: The anatomy of a `.feature` file",
                ("section-12-the-anatomy-of-a-feature-file"),
            ),
            ("UPPER case", "upper-case"),
            ("Hyphen-ated words", "hyphen-ated-words"),
            ("Trailing punctuation!?", "trailing-punctuation"),
            ("`code` first", "code-first"),
            ("  padded  ", "padded"),
        ],
    )
    def test_matches_github_slug(self, heading: str, anchor: str) -> None:
        """Headings should slug exactly as GitHub renders them."""
        result = github_heading_anchor(heading)
        assert result == anchor, (
            f"{heading!r} should slug to {anchor!r}, got {result!r}"
        )


class TestHeadingAnchors:
    """Tests for :func:`check_users_guide_links.heading_anchors`."""

    def test_collects_all_heading_levels(self) -> None:
        """Every heading level from h1 to h6 should contribute an anchor."""
        markdown = "# One\n\n## Two\n\n###### Six\n"
        anchors = heading_anchors(markdown)
        assert anchors == {"one", "two", "six"}, (
            f"expected all heading levels, got {anchors}"
        )

    def test_ignores_comments_inside_code_fences(self) -> None:
        """``#`` comments in fenced code blocks are not headings."""
        markdown = "# Real\n\n```bash\n# not a heading\n```\n"
        anchors = heading_anchors(markdown)
        assert anchors == {"real"}, f"fenced comment should be ignored, got {anchors}"

    def test_resumes_after_code_fence_closes(self) -> None:
        """Headings after a closed fence should be collected again."""
        markdown = "```\n# inside\n```\n# After\n"
        anchors = heading_anchors(markdown)
        assert anchors == {"after"}, (
            f"headings after a closed fence should resume, got {anchors}"
        )

    def test_empty_document_yields_no_anchors(self) -> None:
        """A document without headings should produce an empty set."""
        anchors = heading_anchors("just prose\n")
        assert anchors == set(), (
            f"prose without headings should yield no anchors, got {anchors}"
        )


class TestReferenceDefinitions:
    """Tests for :func:`check_users_guide_links.reference_definitions`."""

    def test_extracts_labels_and_urls_in_order(self) -> None:
        """Reference definitions should be returned in document order."""
        markdown = "[b]: https://example.com/b\n[a]: https://example.com/a\n"
        definitions = reference_definitions(markdown)
        assert definitions == [
            ("b", "https://example.com/b"),
            ("a", "https://example.com/a"),
        ], f"definitions should be returned in document order, got {definitions}"

    def test_ignores_inline_links_and_prose(self) -> None:
        """Only ``[label]: url`` lines should match."""
        markdown = "See [inline](https://example.com) links.\n[not a ref] text\n"
        definitions = reference_definitions(markdown)
        assert definitions == [], (
            f"inline links and prose should not match, got {definitions}"
        )

    def test_ignores_indented_reference_like_lines(self) -> None:
        """Lines that do not start at column zero should not match."""
        markdown = "  [label]: https://example.com\n"
        definitions = reference_definitions(markdown)
        assert definitions == [], (
            f"indented reference-like lines should not match, got {definitions}"
        )


class TestCheckRepoLink:
    """Tests for :func:`check_users_guide_links.check_repo_link`."""

    @pytest.fixture
    def repo(self, tmp_path: Path) -> Path:
        """Create a repository root with one docs file."""
        docs = tmp_path / "docs"
        docs.mkdir()
        (docs / "target.md").write_text(
            "# Top\n\n## Section 1.2: Details here\n", encoding="utf-8"
        )
        return tmp_path

    def test_accepts_canonical_link_without_fragment(self, repo: Path) -> None:
        """A canonical link to an existing document is valid."""
        violations = check_repo_link(repo, "ok", f"{BASE_URL}target.md")
        assert not violations, (
            f"canonical link without fragment should be valid, got {violations}"
        )

    def test_accepts_fragment_matching_heading(self, repo: Path) -> None:
        """A fragment matching a heading anchor is valid."""
        url = f"{BASE_URL}target.md#section-12-details-here"
        violations = check_repo_link(repo, "ok", url)
        assert not violations, (
            f"fragment matching a heading should be valid, got {violations}"
        )

    def test_rejects_non_canonical_base(self, repo: Path) -> None:
        """A URL outside the canonical base should be reported."""
        url = "https://github.com/leynos/rstest-bdd/blob/master/docs/target.md"
        violations = check_repo_link(repo, "bad-base", url)
        assert len(violations) == 1, f"expected exactly one violation, got {violations}"
        assert "canonical base URL" in violations[0], (
            f"violation should mention the canonical base URL, got {violations[0]!r}"
        )
        assert "bad-base" in violations[0], (
            f"violation should name the bad-base label, got {violations[0]!r}"
        )

    def test_rejects_missing_document(self, repo: Path) -> None:
        """A link to a document that does not exist should be reported."""
        violations = check_repo_link(repo, "gone", f"{BASE_URL}gone.md")
        assert violations == ["[gone] points at a missing document: docs/gone.md"], (
            f"missing document should be reported, got {violations}"
        )

    def test_rejects_unknown_fragment(self, repo: Path) -> None:
        """A fragment matching no heading should be reported."""
        violations = check_repo_link(repo, "frag", f"{BASE_URL}target.md#nope")
        assert violations == [
            "[frag] fragment #nope matches no heading in docs/target.md"
        ], f"unknown fragment should be reported, got {violations}"


class TestCheckGuide:
    """Tests for :func:`check_users_guide_links.check_guide`."""

    @staticmethod
    def write_guide(root: Path, markdown: str) -> None:
        """Write guide content beneath a temporary repository root."""
        guide = root / GUIDE
        guide.parent.mkdir(parents=True, exist_ok=True)
        guide.write_text(markdown, encoding="utf-8")

    def test_passes_for_valid_repository_links(self, tmp_path: Path) -> None:
        """A guide whose repository links all resolve should pass."""
        (tmp_path / "docs").mkdir()
        (tmp_path / "docs" / "other.md").write_text("# Other\n", encoding="utf-8")
        self.write_guide(
            tmp_path,
            f"[other]: {BASE_URL}other.md\n"
            "[docs-rs]: https://docs.rs/rstest-bdd/latest/\n",
        )
        violations = check_guide(tmp_path)
        assert not violations, f"valid repository links should pass, got {violations}"

    def test_skips_non_repository_links(self, tmp_path: Path) -> None:
        """External links such as docs.rs are not validated."""
        (tmp_path / "docs").mkdir()
        (tmp_path / "docs" / "other.md").write_text("# Other\n", encoding="utf-8")
        self.write_guide(
            tmp_path,
            f"[other]: {BASE_URL}other.md\n"
            "[external]: https://example.com/blob/main/docs/missing.md\n",
        )
        violations = check_guide(tmp_path)
        assert not violations, (
            f"non-repository links should be skipped, got {violations}"
        )

    def test_reports_missing_guide(self, tmp_path: Path) -> None:
        """An absent guide file should be reported, not raised."""
        violations = check_guide(tmp_path)
        assert len(violations) == 1, f"expected exactly one violation, got {violations}"
        assert str(GUIDE) in violations[0], (
            f"violation should name the guide path, got {violations[0]!r}"
        )
        assert "could not read" in violations[0], (
            f"violation should report the read failure, got {violations[0]!r}"
        )

    def test_reports_guide_without_repository_links(self, tmp_path: Path) -> None:
        """A guide with no repository links should fail the tripwire."""
        self.write_guide(tmp_path, "no references here\n")
        violations = check_guide(tmp_path)
        assert len(violations) == 1, f"expected exactly one violation, got {violations}"
        assert "no repository reference links" in violations[0], (
            f"violation should report the missing-links tripwire, got {violations[0]!r}"
        )

    def test_aggregates_violations_across_links(self, tmp_path: Path) -> None:
        """Each invalid reference should contribute its own violation."""
        (tmp_path / "docs").mkdir()
        self.write_guide(
            tmp_path,
            f"[one]: {BASE_URL}missing-one.md\n[two]: {BASE_URL}missing-two.md\n",
        )
        violations = check_guide(tmp_path)
        assert len(violations) == 2, (
            f"each invalid reference should contribute a violation, got {violations}"
        )
        assert any("missing-one.md" in violation for violation in violations), (
            f"violations should mention missing-one.md, got {violations}"
        )
        assert any("missing-two.md" in violation for violation in violations), (
            f"violations should mention missing-two.md, got {violations}"
        )


class TestGithubHeadingAnchorProperties:
    """Property tests for :func:`check_users_guide_links.github_heading_anchor`."""

    @given(heading=st.text())
    def test_output_is_lowercase(self, heading: str) -> None:
        """Anchors should never contain uppercase characters."""
        result = github_heading_anchor(heading)
        assert result == result.lower(), f"anchor should be lowercase, got {result!r}"

    @given(heading=st.text())
    def test_output_contains_no_spaces(self, heading: str) -> None:
        """Every space should have been replaced or stripped."""
        anchor = github_heading_anchor(heading)
        assert " " not in anchor, f"anchor should contain no spaces, got {anchor!r}"

    @given(heading=st.text())
    def test_output_contains_only_word_chars_and_hyphens(self, heading: str) -> None:
        """Anchors should consist solely of word characters and hyphens."""
        anchor = github_heading_anchor(heading)
        assert re.fullmatch(r"[\w\-]*", anchor), (
            f"anchor should be word-chars/hyphens only, got {anchor!r}"
        )

    @given(heading=st.text(alphabet=st.characters(max_codepoint=0x7F)))
    def test_ascii_output_matches_github_slug_alphabet(self, heading: str) -> None:
        """ASCII headings should slug to ``[a-z0-9_-]*`` exactly."""
        anchor = github_heading_anchor(heading)
        assert re.fullmatch(r"[a-z0-9_\-]*", anchor), (
            f"ASCII anchor should match [a-z0-9_-]*, got {anchor!r}"
        )

    @given(heading=st.text())
    def test_idempotent(self, heading: str) -> None:
        """Slugging an existing anchor should not change it."""
        once = github_heading_anchor(heading)
        twice = github_heading_anchor(once)
        assert twice == once, f"slugging should be idempotent: {once!r} -> {twice!r}"


class TestHeadingAnchorsProperties:
    """Property tests for :func:`check_users_guide_links.heading_anchors`."""

    @given(heading=single_line_text)
    def test_top_level_heading_is_collected(self, heading: str) -> None:
        """A lone ``# heading`` line should yield exactly its anchor."""
        markdown = f"# {heading}\n"
        anchors = heading_anchors(markdown)
        expected = {github_heading_anchor(heading)}
        assert anchors == expected, f"expected {expected}, got {anchors}"

    @given(heading=single_line_text)
    def test_fenced_heading_is_ignored(self, heading: str) -> None:
        """A heading inside a balanced code fence should be ignored."""
        markdown = f"```\n# {heading}\n```\n"
        anchors = heading_anchors(markdown)
        assert anchors == set(), f"expected no anchors, got {anchors}"
