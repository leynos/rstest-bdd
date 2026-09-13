"""Unit tests for the users-guide link checker.

The checker validates the guide's reference block; the link identity and
generation it is built on are covered by ``test_users_guide_links.py``.
"""

import typing as typ

import pytest
from check_users_guide_links import check_guide, check_repo_link
from users_guide_links import BASE_URL, GUIDE, REPOSITORY_URL

if typ.TYPE_CHECKING:
    from pathlib import Path

#: A URL against a branch the repository no longer uses, naming a document
#: that does exist: drift the checker must report rather than skip.
STALE_BRANCH_URL = f"{REPOSITORY_URL}/blob/master/docs/target.md"


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
        violations = check_repo_link(repo, "bad-base", STALE_BRANCH_URL)
        assert len(violations) == 1, f"expected exactly one violation, got {violations}"
        assert "canonical base URL" in violations[0], (
            f"violation should mention the canonical base URL, got {violations[0]!r}"
        )
        assert "bad-base" in violations[0], (
            f"violation should name the bad-base label, got {violations[0]!r}"
        )
        assert "make update-users-guide-links" in violations[0], (
            f"violation should name the remedy, got {violations[0]!r}"
        )

    def test_rejects_repository_link_naming_no_document(self, repo: Path) -> None:
        """A repository URL that names no document should be reported."""
        url = f"{REPOSITORY_URL}/issues/537"
        violations = check_repo_link(repo, "issue", url)
        assert len(violations) == 1, f"expected exactly one violation, got {violations}"
        assert "does not name a document" in violations[0], (
            f"violation should report the unrecognized URL, got {violations[0]!r}"
        )

    def test_rejects_missing_document(self, repo: Path) -> None:
        """A link to a document that does not exist should be reported."""
        violations = check_repo_link(repo, "gone", f"{BASE_URL}gone.md")
        assert violations == ["[gone] points at a missing document: docs/gone.md"], (
            f"missing document should be reported, got {violations}"
        )

    @pytest.mark.parametrize("target", ["../README.md", "/etc/passwd"])
    def test_rejects_targets_outside_the_docs_tree(
        self, repo: Path, target: str
    ) -> None:
        """A target that resolves outside ``docs/`` should be reported."""
        (repo / "README.md").write_text("# Root\n", encoding="utf-8")

        violations = check_repo_link(repo, "escape", f"{BASE_URL}{target}")

        assert violations == [
            f"[escape] points outside the docs/ directory: {target}"
        ], f"an escaping target should be reported, got {violations}"

    def test_accepts_target_in_a_docs_subdirectory(self, repo: Path) -> None:
        """A canonical target below the documentation root is still valid."""
        nested = repo / "docs" / "sub"
        nested.mkdir()
        (nested / "deep.md").write_text("# Deep\n", encoding="utf-8")

        violations = check_repo_link(repo, "deep", f"{BASE_URL}sub/deep.md")

        assert not violations, f"an in-tree target should be valid, got {violations}"

    def test_rejects_unreadable_document(self, repo: Path) -> None:
        """A target that cannot be read as a file should be reported."""
        (repo / "docs" / "adir.md").mkdir()

        violations = check_repo_link(repo, "unreadable", f"{BASE_URL}adir.md")

        assert len(violations) == 1, f"expected exactly one violation, got {violations}"
        assert "unreadable document" in violations[0], (
            f"violation should report the read failure, got {violations[0]!r}"
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

    def test_reports_drift_from_a_previous_base_url(self, tmp_path: Path) -> None:
        """A block written before the base URL moved is reported, not skipped."""
        (tmp_path / "docs").mkdir()
        (tmp_path / "docs" / "target.md").write_text("# Top\n", encoding="utf-8")
        self.write_guide(tmp_path, f"[target]: {STALE_BRANCH_URL}\n")
        violations = check_guide(tmp_path)
        assert len(violations) == 1, f"expected exactly one violation, got {violations}"
        assert "canonical base URL" in violations[0], (
            f"drift should be reported as a stale base URL, got {violations[0]!r}"
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
