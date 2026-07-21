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
    from cuprum import CommandResult

SCRIPT = Path(__file__).resolve().parents[1] / "check_users_guide_links.py"

PYTHON = Program(str(Path(sys.executable)))
_PROJECT = ProjectSettings(
    name="check-users-guide-links-tests",
    programs=(PYTHON,),
    documentation_locations=(),
    noise_rules=(),
)
_CATALOGUE = ProgramCatalogue(projects=(_PROJECT,))
_python = sh.make(PYTHON, catalogue=_CATALOGUE)


def run_checker(root: Path) -> CommandResult:
    """Run the link checker against ``root`` and capture its output."""
    return _python(str(SCRIPT), "--root", str(root)).run_sync()


def write_guide(root: Path, markdown: str) -> None:
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
        write_guide(tmp_path, f"[other]: {BASE_URL}other.md#a-section\n")

        result = run_checker(tmp_path)

        assert result.exit_code == 0
        assert not result.stderr

    def test_missing_document_exits_one(self, tmp_path: Path) -> None:
        """A link to an absent document should exit 1 and name it."""
        (tmp_path / "docs").mkdir()
        write_guide(tmp_path, f"[gone]: {BASE_URL}gone.md\n")

        result = run_checker(tmp_path)

        assert result.exit_code == 1
        assert result.stderr is not None
        assert "missing document" in result.stderr
        assert "docs/gone.md" in result.stderr

    def test_guide_without_repository_links_exits_one(self, tmp_path: Path) -> None:
        """A guide with no repository links should trip the tripwire."""
        write_guide(tmp_path, "no references here\n")

        result = run_checker(tmp_path)

        assert result.exit_code == 1
        assert result.stderr is not None
        assert "no repository reference links" in result.stderr

    @pytest.mark.parametrize("flag", ["--help", "-h"])
    def test_help_exits_zero(self, flag: str) -> None:
        """The argparse help output should be reachable and exit 0."""
        result = _python(str(SCRIPT), flag).run_sync()

        assert result.exit_code == 0
        assert result.stdout is not None
        assert "--root" in result.stdout
