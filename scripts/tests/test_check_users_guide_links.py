"""Unit tests for the users-guide link checker."""

from __future__ import annotations

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

if typ.TYPE_CHECKING:
    from pathlib import Path


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
        assert github_heading_anchor(heading) == anchor


class TestHeadingAnchors:
    """Tests for :func:`check_users_guide_links.heading_anchors`."""

    def test_collects_all_heading_levels(self) -> None:
        """Every heading level from h1 to h6 should contribute an anchor."""
        markdown = "# One\n\n## Two\n\n###### Six\n"
        assert heading_anchors(markdown) == {"one", "two", "six"}

    def test_ignores_comments_inside_code_fences(self) -> None:
        """``#`` comments in fenced code blocks are not headings."""
        markdown = "# Real\n\n```bash\n# not a heading\n```\n"
        assert heading_anchors(markdown) == {"real"}

    def test_resumes_after_code_fence_closes(self) -> None:
        """Headings after a closed fence should be collected again."""
        markdown = "```\n# inside\n```\n# After\n"
        assert heading_anchors(markdown) == {"after"}

    def test_empty_document_yields_no_anchors(self) -> None:
        """A document without headings should produce an empty set."""
        assert heading_anchors("just prose\n") == set()


class TestReferenceDefinitions:
    """Tests for :func:`check_users_guide_links.reference_definitions`."""

    def test_extracts_labels_and_urls_in_order(self) -> None:
        """Reference definitions should be returned in document order."""
        markdown = "[b]: https://example.com/b\n[a]: https://example.com/a\n"
        assert reference_definitions(markdown) == [
            ("b", "https://example.com/b"),
            ("a", "https://example.com/a"),
        ]

    def test_ignores_inline_links_and_prose(self) -> None:
        """Only ``[label]: url`` lines should match."""
        markdown = "See [inline](https://example.com) links.\n[not a ref] text\n"
        assert reference_definitions(markdown) == []

    def test_ignores_indented_reference_like_lines(self) -> None:
        """Lines that do not start at column zero should not match."""
        markdown = "  [label]: https://example.com\n"
        assert reference_definitions(markdown) == []


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
        assert not check_repo_link(repo, "ok", f"{BASE_URL}target.md")

    def test_accepts_fragment_matching_heading(self, repo: Path) -> None:
        """A fragment matching a heading anchor is valid."""
        url = f"{BASE_URL}target.md#section-12-details-here"
        assert not check_repo_link(repo, "ok", url)

    def test_rejects_non_canonical_base(self, repo: Path) -> None:
        """A URL outside the canonical base should be reported."""
        url = "https://github.com/leynos/rstest-bdd/blob/master/docs/target.md"
        violations = check_repo_link(repo, "bad-base", url)
        assert len(violations) == 1
        assert "canonical base URL" in violations[0]
        assert "bad-base" in violations[0]

    def test_rejects_missing_document(self, repo: Path) -> None:
        """A link to a document that does not exist should be reported."""
        violations = check_repo_link(repo, "gone", f"{BASE_URL}gone.md")
        assert violations == ["[gone] points at a missing document: docs/gone.md"]

    def test_rejects_unknown_fragment(self, repo: Path) -> None:
        """A fragment matching no heading should be reported."""
        violations = check_repo_link(repo, "frag", f"{BASE_URL}target.md#nope")
        assert violations == [
            "[frag] fragment #nope matches no heading in docs/target.md"
        ]


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
        assert not check_guide(tmp_path)

    def test_skips_non_repository_links(self, tmp_path: Path) -> None:
        """External links such as docs.rs are not validated."""
        (tmp_path / "docs").mkdir()
        (tmp_path / "docs" / "other.md").write_text("# Other\n", encoding="utf-8")
        self.write_guide(
            tmp_path,
            f"[other]: {BASE_URL}other.md\n"
            "[external]: https://example.com/blob/main/docs/missing.md\n",
        )
        assert not check_guide(tmp_path)

    def test_reports_missing_guide(self, tmp_path: Path) -> None:
        """An absent guide file should be reported, not raised."""
        violations = check_guide(tmp_path)
        assert len(violations) == 1
        assert str(GUIDE) in violations[0]
        assert "could not read" in violations[0]

    def test_reports_guide_without_repository_links(self, tmp_path: Path) -> None:
        """A guide with no repository links should fail the tripwire."""
        self.write_guide(tmp_path, "no references here\n")
        violations = check_guide(tmp_path)
        assert len(violations) == 1
        assert "no repository reference links" in violations[0]

    def test_aggregates_violations_across_links(self, tmp_path: Path) -> None:
        """Each invalid reference should contribute its own violation."""
        (tmp_path / "docs").mkdir()
        self.write_guide(
            tmp_path,
            f"[one]: {BASE_URL}missing-one.md\n[two]: {BASE_URL}missing-two.md\n",
        )
        violations = check_guide(tmp_path)
        assert len(violations) == 2
        assert any("missing-one.md" in violation for violation in violations)
        assert any("missing-two.md" in violation for violation in violations)
