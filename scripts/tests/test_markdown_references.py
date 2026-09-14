"""Unit tests for the Markdown heading and reference-definition helpers."""

import re

import pytest
from hypothesis import given
from hypothesis import strategies as st
from markdown_references import (
    github_heading_anchor,
    heading_anchors,
    reference_definitions,
)

# Characters at which str.splitlines() breaks a line. Heading strategies
# exclude them so a generated heading stays on a single Markdown line.
LINE_BREAKS: str = "\n\r\x0b\x0c\x1c\x1d\x1e\x85\u2028\u2029"

single_line_text: st.SearchStrategy[str] = st.text(
    alphabet=st.characters(exclude_characters=LINE_BREAKS),
    max_size=80,
)


class TestGithubHeadingAnchor:
    """Tests for :func:`markdown_references.github_heading_anchor`."""

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
            ("_Helpful_", "helpful"),
            ("__Bold__", "bold"),
            ("**Bold** heading", "bold-heading"),
            ("_foo_bar_", "foo_bar"),
            ("foo_bar", "foo_bar"),
            ("snake_case_", "snake_case_"),
            ("[Link](https://example.com) text", "link-text"),
            ("![alt](image.png) text", "alt-text"),
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
    """Tests for :func:`markdown_references.heading_anchors`."""

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

    def test_ignores_comments_inside_tilde_fences(self) -> None:
        """A tilde fence is a code block like any other."""
        markdown = "# Real\n\n~~~bash\n# not a heading\n~~~\n"
        anchors = heading_anchors(markdown)
        assert anchors == {"real"}, (
            f"tilde-fenced comment should be ignored, got {anchors}"
        )

    def test_resumes_after_tilde_fence_closes(self) -> None:
        """Headings after a closed tilde fence should be collected again."""
        markdown = "~~~\n# inside\n~~~\n# After\n"
        anchors = heading_anchors(markdown)
        assert anchors == {"after"}, (
            f"headings after a closed tilde fence should resume, got {anchors}"
        )

    def test_differing_fence_character_is_content(self) -> None:
        """A fence line of the other character does not close the block."""
        markdown = "~~~\n```\n# inside\n~~~\n# After\n"
        anchors = heading_anchors(markdown)
        assert anchors == {"after"}, (
            f"a backtick line should not close a tilde fence, got {anchors}"
        )

    def test_shorter_run_does_not_close_a_longer_fence(self) -> None:
        """A closing fence must be at least as long as the opening run."""
        markdown = "````\n```\n# inside\n````\n# After\n"
        anchors = heading_anchors(markdown)
        assert anchors == {"after"}, f"a shorter run should stay content, got {anchors}"

    def test_info_string_does_not_close_a_fence(self) -> None:
        """A fence line carrying an info string is content, not a closer."""
        markdown = "```\n```python\n# inside\n```\n# After\n"
        anchors = heading_anchors(markdown)
        assert anchors == {"after"}, (
            f"an info string should not close a fence, got {anchors}"
        )

    def test_indented_fence_is_not_a_fence(self) -> None:
        """Four spaces of indentation make an indented code block, not a fence."""
        markdown = "    ```\n# heading\n"
        anchors = heading_anchors(markdown)
        assert anchors == {"heading"}, (
            f"an indented run should not open a fence, got {anchors}"
        )

    def test_empty_document_yields_no_anchors(self) -> None:
        """A document without headings should produce an empty set."""
        anchors = heading_anchors("just prose\n")
        assert anchors == set(), (
            f"prose without headings should yield no anchors, got {anchors}"
        )

    def test_repeated_headings_take_incrementing_suffixes(self) -> None:
        """Repeated headings are anchored in document order, as GitHub does."""
        markdown = "# Setup\n\n## Setup\n\n### Setup\n"
        anchors = heading_anchors(markdown)
        assert anchors == {"setup", "setup-1", "setup-2"}, (
            f"repeated headings should be suffixed in document order, got {anchors}"
        )

    def test_generated_suffix_is_suffixed_again(self) -> None:
        """A slug generated for an earlier heading is claimed and suffixed again."""
        markdown = "# Setup\n\n## Setup\n\n### Setup 1\n"
        anchors = heading_anchors(markdown)
        assert anchors == {"setup", "setup-1", "setup-1-1"}, (
            f"a generated slug should be suffixed again rather than renumbered, "
            f"got {anchors}"
        )


class TestReferenceDefinitions:
    """Tests for :func:`markdown_references.reference_definitions`."""

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


class TestGithubHeadingAnchorProperties:
    """Property tests for :func:`markdown_references.github_heading_anchor`."""

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
    """Property tests for :func:`markdown_references.heading_anchors`."""

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

    @given(heading=single_line_text)
    def test_tilde_fenced_heading_is_ignored(self, heading: str) -> None:
        """A heading inside a balanced tilde fence should be ignored."""
        markdown = f"~~~\n# {heading}\n~~~\n"
        anchors = heading_anchors(markdown)
        assert anchors == set(), f"expected no anchors, got {anchors}"
