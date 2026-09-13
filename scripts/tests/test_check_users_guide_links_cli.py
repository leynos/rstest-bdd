"""CLI integration tests for the users-guide link checker.

These tests invoke the ``scripts/check_users_guide_links.py`` entry point,
exercising the ``--root`` and ``--fix`` options and the exit codes rather than
the helper functions (which have their own unit tests).
"""

import os
import typing as typ

import pytest
from check_users_guide_links import main
from users_guide_links import BASE_URL, GUIDE, REPOSITORY_URL

if typ.TYPE_CHECKING:
    from pathlib import Path

STALE_BRANCH_URL = f"{REPOSITORY_URL}/blob/master/docs/other.md"
CANONICAL_OTHER = f"{BASE_URL}other.md"


def _run_checker(root: Path, *args: str) -> int:
    """Run the link checker entry point against ``root``."""
    return main(("--root", str(root), *args))


def _write_document(root: Path, name: str, markdown: str) -> None:
    """Write a document beneath a temporary repository root."""
    document = root / "docs" / name
    document.parent.mkdir(parents=True, exist_ok=True)
    document.write_text(markdown, encoding="utf-8")


def _write_guide(root: Path, markdown: str) -> None:
    """Write guide content beneath a temporary repository root."""
    guide = root / GUIDE
    guide.parent.mkdir(parents=True, exist_ok=True)
    guide.write_text(markdown, encoding="utf-8")


def _read_guide(root: Path) -> str:
    """Return guide content beneath a temporary repository root."""
    return (root / GUIDE).read_text(encoding="utf-8")


class TestMain:
    """End-to-end tests for the script's command-line entry point."""

    def test_valid_guide_exits_zero(
        self, tmp_path: Path, capsys: pytest.CaptureFixture[str]
    ) -> None:
        """A guide whose repository links all resolve should exit 0."""
        _write_document(tmp_path, "other.md", "# Other\n\n## A section\n")
        _write_guide(tmp_path, f"[other]: {CANONICAL_OTHER}#a-section\n")

        exit_code = _run_checker(tmp_path)
        captured = capsys.readouterr()

        assert exit_code == 0, f"expected exit 0, got {exit_code}: {captured.err}"
        assert not captured.err, f"expected no stderr, got: {captured.err}"

    def test_missing_document_exits_one(
        self, tmp_path: Path, capsys: pytest.CaptureFixture[str]
    ) -> None:
        """A link to an absent document should exit 1 and name it."""
        (tmp_path / "docs").mkdir()
        _write_guide(tmp_path, f"[gone]: {BASE_URL}gone.md\n")

        exit_code = _run_checker(tmp_path)
        captured = capsys.readouterr()

        assert exit_code == 1, f"expected exit 1, got {exit_code}: {captured.err}"
        assert captured.err == (
            "[gone] points at a missing document: docs/gone.md\n"
        ), f"stderr should pin the missing-document diagnostic: {captured.err}"

    def test_rejects_a_stale_base_url(
        self, tmp_path: Path, capsys: pytest.CaptureFixture[str]
    ) -> None:
        """A definition written against an earlier base URL should exit 1."""
        _write_document(tmp_path, "other.md", "# Other\n")
        _write_guide(tmp_path, f"[other]: {STALE_BRANCH_URL}\n")

        exit_code = _run_checker(tmp_path)
        captured = capsys.readouterr()

        assert exit_code == 1, f"expected exit 1, got {exit_code}: {captured.err}"
        assert captured.err == (
            f"[other] does not use the canonical base URL {BASE_URL}: "
            f"{STALE_BRANCH_URL} (run: make update-users-guide-links)\n"
        ), f"stderr should pin the diagnostic: {captured.err}"

    def test_guide_without_repository_links_exits_one(
        self, tmp_path: Path, capsys: pytest.CaptureFixture[str]
    ) -> None:
        """A guide with no repository links should trip the tripwire."""
        _write_guide(tmp_path, "no references here\n")

        exit_code = _run_checker(tmp_path)
        captured = capsys.readouterr()

        assert exit_code == 1, f"expected exit 1, got {exit_code}: {captured.err}"
        assert captured.err == (
            f"no repository reference links found in {GUIDE}; "
            "the reference block may have been removed or reformatted\n"
        ), f"stderr should pin the tripwire diagnostic: {captured.err}"

    @pytest.mark.parametrize("flag", ["--help", "-h"])
    def test_help_exits_zero(
        self, flag: str, capsys: pytest.CaptureFixture[str]
    ) -> None:
        """The argparse help output should be reachable and exit 0."""
        with pytest.raises(SystemExit) as exc_info:
            main((flag,))
        captured = capsys.readouterr()

        assert exc_info.value.code == 0, (
            f"expected exit 0, got {exc_info.value.code}: {captured.err}"
        )
        assert "--root" in captured.out, f"help should document --root: {captured.out}"
        assert "--fix" in captured.out, f"help should document --fix: {captured.out}"

    def test_default_root_checks_repository_guide(
        self, capsys: pytest.CaptureFixture[str]
    ) -> None:
        """Omitting --root falls back to the script-relative repository root.

        The default is ``Path(__file__).resolve().parents[1]`` rather than the
        current directory, so every other test passes ``--root`` and never
        exercises that fallback. The checked-in users' guide is kept
        link-clean, so running with no options must validate it and exit 0.
        """
        exit_code = main(())
        captured = capsys.readouterr()

        assert exit_code == 0, (
            f"default-root run should validate the repository guide, got "
            f"{exit_code}: {captured.err}"
        )


class TestFix:
    """End-to-end tests for the ``--fix`` regeneration mode."""

    def test_rewrites_a_stale_reference_block(
        self, tmp_path: Path, capsys: pytest.CaptureFixture[str]
    ) -> None:
        """A block written before the base URL moved is regenerated."""
        _write_document(tmp_path, "other.md", "# Other\n")
        _write_guide(tmp_path, f"[other]: {STALE_BRANCH_URL}\n")

        exit_code = _run_checker(tmp_path, "--fix")
        captured = capsys.readouterr()

        assert exit_code == 0, f"expected exit 0, got {exit_code}: {captured.err}"
        assert not captured.err, f"expected no stderr, got: {captured.err}"
        assert captured.out == f"rewrote 1 reference line(s) in {GUIDE}\n", (
            f"stdout should report the rewrite: {captured.out}"
        )
        assert _read_guide(tmp_path) == f"[other]: {CANONICAL_OTHER}\n", (
            f"guide should hold canonical links, got {_read_guide(tmp_path)!r}"
        )

    def test_reports_when_nothing_needs_rewriting(
        self, tmp_path: Path, capsys: pytest.CaptureFixture[str]
    ) -> None:
        """An already canonical block is left alone."""
        _write_document(tmp_path, "other.md", "# Other\n")
        _write_guide(tmp_path, f"[other]: {CANONICAL_OTHER}\n")

        exit_code = _run_checker(tmp_path, "--fix")
        captured = capsys.readouterr()

        assert exit_code == 0, f"expected exit 0, got {exit_code}: {captured.err}"
        assert captured.out == (
            f"{GUIDE} already matches the generated reference links\n"
        ), f"stdout should report that nothing changed: {captured.out}"
        assert _read_guide(tmp_path) == f"[other]: {CANONICAL_OTHER}\n", (
            f"guide should be untouched, got {_read_guide(tmp_path)!r}"
        )

    def test_rewrites_only_the_stale_lines(
        self, tmp_path: Path, capsys: pytest.CaptureFixture[str]
    ) -> None:
        """Prose and third-party definitions survive regeneration verbatim."""
        _write_document(tmp_path, "other.md", "# Other\n")
        markdown = (
            "# Users guide\n\nSee [other][other] and [docs-rs][docs-rs].\n\n"
            f"[other]: {STALE_BRANCH_URL}\n"
            "[docs-rs]: https://docs.rs/rstest-bdd/latest/\n"
            f"[external]: https://example.com/blob/main/docs/other.md\n"
        )
        _write_guide(tmp_path, markdown)

        exit_code = _run_checker(tmp_path, "--fix")
        captured = capsys.readouterr()

        assert exit_code == 0, f"expected exit 0, got {exit_code}: {captured.err}"
        expected = markdown.replace(STALE_BRANCH_URL, CANONICAL_OTHER)
        assert _read_guide(tmp_path) == expected, (
            f"only the stale line should change, got {_read_guide(tmp_path)!r}"
        )

    def test_still_reports_a_missing_document(
        self, tmp_path: Path, capsys: pytest.CaptureFixture[str]
    ) -> None:
        """Regeneration must not mask a canonical link to an absent document."""
        (tmp_path / "docs").mkdir()
        markdown = f"[gone]: {BASE_URL}gone.md\n"
        _write_guide(tmp_path, markdown)

        exit_code = _run_checker(tmp_path, "--fix")
        captured = capsys.readouterr()

        assert exit_code == 1, f"expected exit 1, got {exit_code}: {captured.err}"
        assert "missing document" in captured.err, (
            f"stderr should report the missing document: {captured.err}"
        )
        assert _read_guide(tmp_path) == markdown, "the guide should be unchanged"

    def test_reports_a_reference_it_cannot_rewrite(
        self, tmp_path: Path, capsys: pytest.CaptureFixture[str]
    ) -> None:
        """A repository URL that names no document is reported, not rewritten."""
        markdown = f"[issue]: {REPOSITORY_URL}/issues/537\n"
        _write_guide(tmp_path, markdown)

        exit_code = _run_checker(tmp_path, "--fix")
        captured = capsys.readouterr()

        assert exit_code == 1, f"expected exit 1, got {exit_code}: {captured.err}"
        assert captured.err == (
            f"[issue] does not name a document under the canonical base URL "
            f"{BASE_URL}: {REPOSITORY_URL}/issues/537\n"
        ), f"stderr should pin the unrecognized-link diagnostic: {captured.err}"
        assert _read_guide(tmp_path) == markdown, "the guide should be unchanged"

    def test_reports_a_missing_guide(
        self, tmp_path: Path, capsys: pytest.CaptureFixture[str]
    ) -> None:
        """An absent guide is reported rather than created."""
        exit_code = _run_checker(tmp_path, "--fix")
        captured = capsys.readouterr()

        assert exit_code == 1, f"expected exit 1, got {exit_code}: {captured.err}"
        assert captured.err.startswith(f"could not read {GUIDE}: "), (
            f"stderr should pin the read-failure diagnostic for {GUIDE}: {captured.err}"
        )

    def test_reports_a_guide_it_cannot_write(
        self, tmp_path: Path, capsys: pytest.CaptureFixture[str]
    ) -> None:
        """A failed write should be reported rather than raised."""
        _write_document(tmp_path, "other.md", "# Other\n")
        _write_guide(tmp_path, f"[other]: {STALE_BRANCH_URL}\n")
        guide = tmp_path / GUIDE
        guide.chmod(0o444)
        if os.access(guide, os.W_OK):
            pytest.skip("file modes do not restrict this user")

        exit_code = _run_checker(tmp_path, "--fix")
        captured = capsys.readouterr()

        assert exit_code == 1, f"expected exit 1, got {exit_code}: {captured.err}"
        assert captured.err.startswith(f"could not write {GUIDE}: "), (
            f"stderr should report the write failure: {captured.err}"
        )

    def test_preserves_crlf_line_endings(
        self, tmp_path: Path, capsys: pytest.CaptureFixture[str]
    ) -> None:
        """A rewrite must not reflow a CRLF guide into LF."""
        _write_document(tmp_path, "other.md", "# Other\n")
        guide = tmp_path / GUIDE
        guide.parent.mkdir(parents=True, exist_ok=True)
        guide.write_bytes(f"[other]: {STALE_BRANCH_URL}\r\n".encode())

        exit_code = _run_checker(tmp_path, "--fix")
        captured = capsys.readouterr()

        assert exit_code == 0, f"expected exit 0, got {exit_code}: {captured.err}"
        expected = f"[other]: {CANONICAL_OTHER}\r\n".encode()
        assert guide.read_bytes() == expected, (
            f"every line ending should survive, got {guide.read_bytes()!r}"
        )
