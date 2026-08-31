"""CLI integration tests for the users-guide link checker.

These tests invoke the ``scripts/check_users_guide_links.py`` entry point,
exercising the ``--root`` option and exit codes rather than the helper
functions (which have their own unit tests).
"""

from pathlib import Path

import pytest
from check_users_guide_links import BASE_URL, GUIDE, main


def _run_checker(root: Path) -> int:
    """Run the link checker entry point against ``root``."""
    return main(("--root", str(root)))


def _write_guide(root: Path, markdown: str) -> None:
    """Write guide content beneath a temporary repository root."""
    guide = root / GUIDE
    guide.parent.mkdir(parents=True, exist_ok=True)
    guide.write_text(markdown, encoding="utf-8")


class TestMain:
    """End-to-end tests for the script's command-line entry point."""

    def test_valid_guide_exits_zero(
        self, tmp_path: Path, capsys: pytest.CaptureFixture[str]
    ) -> None:
        """A guide whose repository links all resolve should exit 0."""
        (tmp_path / "docs" / "other.md").parent.mkdir(parents=True, exist_ok=True)
        (tmp_path / "docs" / "other.md").write_text(
            "# Other\n\n## A section\n", encoding="utf-8"
        )
        _write_guide(tmp_path, f"[other]: {BASE_URL}other.md#a-section\n")

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
        assert "missing document" in captured.err, (
            f"stderr should mention the missing document: {captured.err}"
        )
        assert "docs/gone.md" in captured.err, (
            f"stderr should name docs/gone.md: {captured.err}"
        )

    def test_guide_without_repository_links_exits_one(
        self, tmp_path: Path, capsys: pytest.CaptureFixture[str]
    ) -> None:
        """A guide with no repository links should trip the tripwire."""
        _write_guide(tmp_path, "no references here\n")

        exit_code = _run_checker(tmp_path)
        captured = capsys.readouterr()

        assert exit_code == 1, f"expected exit 1, got {exit_code}: {captured.err}"
        assert "no repository reference links" in captured.err, (
            f"stderr should report the missing-links tripwire: {captured.err}"
        )

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
