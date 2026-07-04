"""Unit tests for the `#[serial]`/nextest matrix drift checker."""

from __future__ import annotations

import runpy
from pathlib import Path

import pytest
from check_serial_nextest_matrix import (
    DESIGN_DOC,
    MATRIX_HEADING,
    USERS_GUIDE,
    check_matrix_tables,
    extract_matrix_rows,
    normalise_table_row,
)

TABLE = "\n".join([
    "| Runner | `#[serial]` effect | Cross-process exclusivity |",
    "| --- | --- | --- |",
    "| `cargo test` | In-process mutex; required | Not provided by `#[serial]` |",
    (
        "| nextest (process-per-test) | Redundant-but-harmless | "
        "`#[file_serial]` or test-group |"
    ),
    "",
])


def document(heading: str, table: str = TABLE) -> str:
    """Build a minimal document containing the runner matrix."""
    return f"# Top\n\n## {heading}\n\nIntro.\n\n{table}\n\n## Next\n"


def write_repo_docs(
    root: Path, users_table: str = TABLE, design_table: str = TABLE
) -> None:
    """Write both documents beneath a temporary repository root."""
    (root / USERS_GUIDE.parent).mkdir(parents=True, exist_ok=True)
    (root / DESIGN_DOC.parent).mkdir(parents=True, exist_ok=True)
    (root / USERS_GUIDE).write_text(
        document(MATRIX_HEADING, users_table), encoding="utf-8"
    )
    (root / DESIGN_DOC).write_text(
        document(MATRIX_HEADING, design_table), encoding="utf-8"
    )


class TestNormaliseTableRow:
    """Tests for :func:`check_serial_nextest_matrix.normalise_table_row`."""

    def test_collapses_whitespace_runs(self) -> None:
        """Spacing-only table alignment changes should disappear."""
        row = "| a |  b   c |"
        assert normalise_table_row(row) == "| a | b c |", (
            "table row normalization should collapse spacing-only changes"
        )


class TestExtractMatrixRows:
    """Tests for :func:`check_serial_nextest_matrix.extract_matrix_rows`."""

    def test_extracts_two_data_rows_from_anchored_section(self) -> None:
        """The table anchored below the requested heading should be read."""
        rows = extract_matrix_rows(document(MATRIX_HEADING), MATRIX_HEADING)
        assert len(rows) == 2, "matrix extraction should return both data rows"
        assert rows[1].startswith("| nextest (process-per-test) |"), (
            "matrix extraction should preserve the nextest row"
        )

    def test_accepts_numbered_heading_prefix(self) -> None:
        """Design headings may include section numbers before the anchor text."""
        markdown = document(f"2.7.6.7 {MATRIX_HEADING}")
        rows = extract_matrix_rows(markdown, MATRIX_HEADING)
        assert len(rows) == 2, (
            "matrix extraction should accept numbered design heading prefixes"
        )

    def test_reports_missing_heading(self) -> None:
        """A missing anchor heading is an explicit error."""
        with pytest.raises(ValueError, match="heading not found"):
            extract_matrix_rows(document("Different heading"), MATRIX_HEADING)

    def test_reports_missing_table(self) -> None:
        """A section without the runner matrix should be rejected."""
        with pytest.raises(ValueError, match="runner matrix not found"):
            extract_matrix_rows(
                "# Top\n\n## Test-runner parallelism and scenario state\n",
                MATRIX_HEADING,
            )

    def test_reports_missing_separator(self) -> None:
        """A runner table without a separator row should be rejected."""
        malformed_table = "\n".join([
            "| Runner | `#[serial]` effect | Cross-process exclusivity |",
            (
                "| `cargo test` | In-process mutex; required | "
                "Not provided by `#[serial]` |"
            ),
            "",
        ])
        with pytest.raises(ValueError, match="has no separator row"):
            extract_matrix_rows(
                document(MATRIX_HEADING, malformed_table), MATRIX_HEADING
            )


class TestCheckMatrixTables:
    """Tests for :func:`check_serial_nextest_matrix.check_matrix_tables`."""

    def test_passes_for_identical_data_rows(self, tmp_path: Path) -> None:
        """Identical runner-matrix rows should pass."""
        write_repo_docs(tmp_path)
        assert not check_matrix_tables(tmp_path), (
            "identical user-guide and design matrix rows should pass"
        )

    def test_reports_content_mutation(self, tmp_path: Path) -> None:
        """A changed data cell should produce a row-specific violation."""
        mutated = TABLE.replace("Redundant-but-harmless", "Required")
        write_repo_docs(tmp_path, users_table=mutated)

        violations = check_matrix_tables(tmp_path)

        assert violations[0] == "`#[serial]`/nextest runner matrix data rows differ:", (
            "content drift should report a matrix row mismatch header"
        )
        assert "row 2:" in violations, "content drift should identify the changed row"
        assert any(str(USERS_GUIDE) in violation for violation in violations), (
            "content drift should identify the changed user-guide source"
        )

    def test_ignores_whitespace_only_mutation(self, tmp_path: Path) -> None:
        """Extra table alignment whitespace must not count as drift."""
        mutated = TABLE.replace(
            "| nextest (process-per-test) |",
            "|   nextest   (process-per-test)   |",
        )
        write_repo_docs(tmp_path, users_table=mutated)

        assert not check_matrix_tables(tmp_path), (
            "whitespace-only matrix alignment changes should not count as drift"
        )

    def test_accepts_aligned_markdown_table(self, tmp_path: Path) -> None:
        """Markdown formatter alignment should not hide the runner matrix."""
        aligned_table = "\n".join([
            (
                "| Runner                     | `#[serial]` effect         | "
                "Cross-process exclusivity      |"
            ),
            (
                "| -------------------------- | -------------------------- | "
                "------------------------------ |"
            ),
            (
                "| `cargo test`               | In-process mutex; required | "
                "Not provided by `#[serial]`    |"
            ),
            (
                "| nextest (process-per-test) | Redundant-but-harmless     | "
                "`#[file_serial]` or test-group |"
            ),
            "",
        ])
        write_repo_docs(tmp_path, users_table=aligned_table, design_table=aligned_table)

        assert not check_matrix_tables(tmp_path), (
            "aligned Markdown tables should still match after normalization"
        )

    def test_reports_missing_table_anchor(self, tmp_path: Path) -> None:
        """A missing heading should fail rather than silently skipping a table."""
        write_repo_docs(tmp_path)
        (tmp_path / USERS_GUIDE).write_text(
            document("Different heading"), encoding="utf-8"
        )

        violations = check_matrix_tables(tmp_path)

        assert len(violations) == 1, (
            "missing matrix anchor should produce exactly one violation"
        )
        assert "heading not found" in violations[0], (
            "missing matrix anchor should explain the missing heading"
        )

    def test_reports_removed_table_row(self, tmp_path: Path) -> None:
        """Deleting a matrix row must fail instead of weakening the matcher."""
        removed_row = TABLE.replace(
            (
                "| nextest (process-per-test) | Redundant-but-harmless | "
                "`#[file_serial]` or test-group |\n"
            ),
            "",
        )
        write_repo_docs(tmp_path, users_table=removed_row)

        violations = check_matrix_tables(tmp_path)

        assert len(violations) == 1, (
            "removing a matrix row should produce exactly one violation"
        )
        assert "has 1 data rows; expected 2" in violations[0], (
            "removing a matrix row should report the shortened table"
        )

    def test_reports_unreadable_document(self, tmp_path: Path) -> None:
        """Missing documents should surface the read failure."""
        write_repo_docs(tmp_path)
        (tmp_path / USERS_GUIDE).unlink()

        violations = check_matrix_tables(tmp_path)

        assert len(violations) == 1, (
            "an unreadable matrix document should produce exactly one violation"
        )
        assert f"could not read {USERS_GUIDE}" in violations[0], (
            "an unreadable matrix document should identify the failed path"
        )


class TestCli:
    """Behavioural tests for the script entrypoint."""

    def test_cli_passes_for_repository_docs(
        self, capsys: pytest.CaptureFixture[str]
    ) -> None:
        """The command-line script should pass against the repository docs."""
        with pytest.raises(SystemExit) as exc_info:
            runpy.run_path(
                "scripts/check_serial_nextest_matrix.py", run_name="__main__"
            )

        captured = capsys.readouterr()

        assert exc_info.value.code == 0, (
            "CLI should accept the checked-in users-guide and design matrices"
        )
        assert not captured.err, "CLI should not report violations on success"


class TestMakefileHook:
    """Tests for the Makefile lint integration."""

    def test_make_lint_runs_serial_matrix_checker(self) -> None:
        """The lint target should exercise the serial/nextest matrix checker."""
        makefile = Path("Makefile").read_text(encoding="utf-8")

        assert "python3 scripts/check_serial_nextest_matrix.py" in makefile, (
            "make lint should run the serial/nextest matrix checker"
        )
