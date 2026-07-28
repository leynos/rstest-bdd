"""CLI integration tests for the users-guide link checker.

These tests invoke ``scripts/check_users_guide_links.py`` as a subprocess
via cuprum, exercising the ``--root`` option and the process exit codes
rather than the helper functions (which have their own unit tests).
"""

from __future__ import annotations

import sys
import typing as typ
from pathlib import Path

import pytest
from check_users_guide_links import BASE_URL, GUIDE
from cuprum import Program, ProgramCatalogue, ProjectSettings, sh

if typ.TYPE_CHECKING:
    from cuprum import CommandResult, SafeCmdBuilder

SCRIPT: Path = Path(__file__).resolve().parents[1] / "check_users_guide_links.py"

PYTHON: Program = Program(str(Path(sys.executable)))
_PROJECT: ProjectSettings = ProjectSettings(
    name="check-users-guide-links-tests",
    programs=(PYTHON,),
    documentation_locations=(),
    noise_rules=(),
)
_CATALOGUE: ProgramCatalogue = ProgramCatalogue(projects=(_PROJECT,))
_python: SafeCmdBuilder = sh.make(PYTHON, catalogue=_CATALOGUE)


def _run_checker(root: Path) -> CommandResult:
    """Run the link checker against ``root`` and capture its output."""
    return _python(str(SCRIPT), "--root", str(root)).run_sync()


def _write_guide(root: Path, markdown: str) -> None:
    """Write guide content beneath a temporary repository root."""
    guide = root / GUIDE
    guide.parent.mkdir(parents=True, exist_ok=True)
    guide.write_text(markdown, encoding="utf-8")


class TestMain:
    """End-to-end tests for the script's command-line entry point."""

    def test_valid_guide_exits_zero(self, tmp_path: Path) -> None:
        """A guide whose repository links all resolve should exit 0."""
        (tmp_path / "docs" / "other.md").parent.mkdir(parents=True, exist_ok=True)
        (tmp_path / "docs" / "other.md").write_text(
            "# Other\n\n## A section\n", encoding="utf-8"
        )
        _write_guide(tmp_path, f"[other]: {BASE_URL}other.md#a-section\n")

        result = _run_checker(tmp_path)

        assert result.exit_code == 0, (
            f"expected exit 0, got {result.exit_code}: {result.stderr}"
        )
        assert not result.stderr, f"expected no stderr, got: {result.stderr}"

    def test_missing_document_exits_one(self, tmp_path: Path) -> None:
        """A link to an absent document should exit 1 and name it."""
        (tmp_path / "docs").mkdir()
        _write_guide(tmp_path, f"[gone]: {BASE_URL}gone.md\n")

        result = _run_checker(tmp_path)

        assert result.exit_code == 1, (
            f"expected exit 1, got {result.exit_code}: {result.stderr}"
        )
        assert result.stderr is not None, "expected stderr output"
        assert "missing document" in result.stderr, (
            f"stderr should mention the missing document: {result.stderr}"
        )
        assert "docs/gone.md" in result.stderr, (
            f"stderr should name docs/gone.md: {result.stderr}"
        )

    def test_guide_without_repository_links_exits_one(self, tmp_path: Path) -> None:
        """A guide with no repository links should trip the tripwire."""
        _write_guide(tmp_path, "no references here\n")

        result = _run_checker(tmp_path)

        assert result.exit_code == 1, (
            f"expected exit 1, got {result.exit_code}: {result.stderr}"
        )
        assert result.stderr is not None, "expected stderr output"
        assert "no repository reference links" in result.stderr, (
            f"stderr should report the missing-links tripwire: {result.stderr}"
        )

    @pytest.mark.parametrize("flag", ["--help", "-h"])
    def test_help_exits_zero(self, flag: str) -> None:
        """The argparse help output should be reachable and exit 0."""
        result = _python(str(SCRIPT), flag).run_sync()

        assert result.exit_code == 0, (
            f"expected exit 0, got {result.exit_code}: {result.stderr}"
        )
        assert result.stdout is not None, "expected help output on stdout"
        assert "--root" in result.stdout, (
            f"help should document --root: {result.stdout}"
        )

    def test_default_root_checks_repository_guide(self) -> None:
        """Omitting --root falls back to the script-relative repository root.

        The default is ``Path(__file__).resolve().parents[1]`` rather than the
        current directory, so every other test passes ``--root`` and never
        exercises that fallback. The checked-in users' guide is kept
        link-clean, so running with no options must validate it and exit 0.
        """
        result = _python(str(SCRIPT)).run_sync()

        assert result.exit_code == 0, (
            f"default-root run should validate the repository guide, got "
            f"{result.exit_code}: {result.stderr}"
        )
