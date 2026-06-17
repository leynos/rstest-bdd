"""Unit tests for the GPUI mapping-table drift checker."""

from __future__ import annotations

import typing as typ

import pytest
from check_gpui_mapping_table import (
    DESIGN_DOC,
    DESIGN_HEADING,
    USERS_GUIDE,
    USERS_HEADING,
    check_mapping_tables,
    extract_mapping_rows,
    normalise_table_row,
)

if typ.TYPE_CHECKING:
    from pathlib import Path


TABLE = "\n".join([
    (
        "> | Operation | Vendored gpui (regression suite + these snippets) | "
        "Published `gpui 0.2.2` (downstream adopters) |"
    ),
    "> | --- | --- | --- |",
    (
        "> | `add_window_view` closure | `\\|_context\\| View::default()` "
        "(one argument) | `\\|_window, view_cx\\| View::new(view_cx)` "
        "(two arguments) |"
    ),
    (
        "> | obtain window handle | `visual_cx.window_handle()` (inherent "
        "method on `VisualTestContext`) | `vcx.window_handle()` (same call, "
        "but `window_handle` is a `VisualContext` trait method, so add "
        "`use gpui::VisualContext;`) |"
    ),
    (
        "> | `VisualTestContext::from_window` | returns "
        "`Option<VisualTestContext>` (`.unwrap_or_else`/`.ok_or`) | returns "
        "`VisualTestContext` by value (no `Option`) |"
    ),
    (
        "> | `read_entity` / `update_entity` | `Option`/`Result` wrappers "
        "(`Some(1)`, `Ok(())`) | identity `type Result<T> = T`; returns `R` "
        "directly |"
    ),
    "",
])


def document(heading: str, table: str = TABLE) -> str:
    """Build a minimal document containing the mapping table."""
    return f"# Top\n\n## {heading}\n\nIntro.\n\n{table}\n\n## Next\n"


def write_repo_docs(
    root: Path, users_table: str = TABLE, design_table: str = TABLE
) -> None:
    """Write both documents beneath a temporary repository root."""
    (root / USERS_GUIDE.parent).mkdir(parents=True, exist_ok=True)
    (root / USERS_GUIDE).write_text(
        document(USERS_HEADING, users_table), encoding="utf-8"
    )
    (root / DESIGN_DOC).write_text(
        document(DESIGN_HEADING, design_table), encoding="utf-8"
    )


class TestNormaliseTableRow:
    """Tests for :func:`check_gpui_mapping_table.normalise_table_row`."""

    def test_collapses_whitespace_runs(self) -> None:
        """Spacing-only table alignment changes should disappear."""
        row = "> | a |  b   c |"
        assert normalise_table_row(row) == "> | a | b c |"


class TestExtractMappingRows:
    """Tests for :func:`check_gpui_mapping_table.extract_mapping_rows`."""

    def test_extracts_four_data_rows_from_anchored_section(self) -> None:
        """The table anchored below the requested heading should be read."""
        rows = extract_mapping_rows(document(DESIGN_HEADING), DESIGN_HEADING)
        assert len(rows) == 4
        assert rows[1].startswith("> | obtain window handle |")

    def test_accepts_numbered_heading_prefix(self) -> None:
        """Design headings may include section numbers before the anchor text."""
        markdown = document(f"2.7.6.2 {DESIGN_HEADING}")
        rows = extract_mapping_rows(markdown, DESIGN_HEADING)
        assert len(rows) == 4

    def test_reports_missing_heading(self) -> None:
        """A missing anchor heading is an explicit error."""
        with pytest.raises(ValueError, match="heading not found"):
            extract_mapping_rows(document("Different heading"), DESIGN_HEADING)

    def test_reports_missing_table(self) -> None:
        """A section without the operation table should be rejected."""
        with pytest.raises(ValueError, match="mapping table not found"):
            extract_mapping_rows(
                "# Top\n\n## Interim GPUI state pattern\n", DESIGN_HEADING
            )


class TestCheckMappingTables:
    """Tests for :func:`check_gpui_mapping_table.check_mapping_tables`."""

    def test_passes_for_identical_data_rows(self, tmp_path: Path) -> None:
        """Identical mapping-table rows should pass."""
        write_repo_docs(tmp_path)
        assert not check_mapping_tables(tmp_path)

    def test_reports_content_mutation(self, tmp_path: Path) -> None:
        """A changed data cell should produce a row-specific violation."""
        mutated = TABLE.replace("same call", "different call")
        write_repo_docs(tmp_path, users_table=mutated)

        violations = check_mapping_tables(tmp_path)

        assert violations[0] == "GPUI mapping table data rows differ:"
        assert "row 2:" in violations
        assert any(str(USERS_GUIDE) in violation for violation in violations)

    def test_ignores_whitespace_only_mutation(self, tmp_path: Path) -> None:
        """Extra table alignment whitespace must not count as drift."""
        mutated = TABLE.replace(
            "| `add_window_view` closure |",
            "|   `add_window_view`   closure   |",
        )
        write_repo_docs(tmp_path, users_table=mutated)

        assert not check_mapping_tables(tmp_path)

    def test_reports_missing_table_anchor(self, tmp_path: Path) -> None:
        """A missing heading should fail rather than silently skipping a table."""
        write_repo_docs(tmp_path)
        (tmp_path / USERS_GUIDE).write_text(
            document("Different heading"), encoding="utf-8"
        )

        violations = check_mapping_tables(tmp_path)

        assert len(violations) == 1
        assert "heading not found" in violations[0]
