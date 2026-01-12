//! Handler utilities for LSP type conversions.
//!
//! This module provides helper functions for converting between `gherkin` types
//! and LSP protocol types, particularly for span and position conversions.

use gherkin::Span;
use lsp_types::{Position, Range};

/// Convert a `gherkin::Span` (byte offsets) to an `lsp_types::Range` (0-based line/col).
///
/// The `gherkin` crate uses byte offsets for spans, while the LSP protocol uses
/// 0-based line and character (column) positions. This function computes the
/// line and column positions by scanning the source text.
///
/// # Arguments
///
/// * `source` - The full source text of the feature file
/// * `span` - The byte offset span to convert
///
/// # Examples
///
/// ```
/// use gherkin::Span;
/// use rstest_bdd_server::handlers::util::gherkin_span_to_lsp_range;
///
/// let source = "Feature: demo\n  Scenario: s\n    Given a step\n";
/// let span = Span { start: 30, end: 42 }; // "Given a step"
/// let range = gherkin_span_to_lsp_range(source, span);
/// assert_eq!(range.start.line, 2);
/// assert_eq!(range.end.line, 2);
/// ```
#[must_use]
pub fn gherkin_span_to_lsp_range(source: &str, span: Span) -> Range {
    let start = byte_offset_to_position(source, span.start);
    let end = byte_offset_to_position(source, span.end);
    Range { start, end }
}

/// A line and column position for internal tracking.
///
/// Used to reduce parameter count in helper functions by grouping related values.
#[derive(Clone, Copy)]
struct LineColPosition {
    line: u32,
    col: u32,
}

/// Calculate UTF-16 code units for a character.
///
/// BMP characters (code points ≤ 0xFFFF) use 1 UTF-16 code unit.
/// Non-BMP characters (code points > 0xFFFF) use 2 UTF-16 code units (surrogate pair).
#[inline]
fn utf16_code_units(ch: char) -> u32 {
    if u32::from(ch) <= 0xFFFF { 1 } else { 2 }
}

/// Return the clamped byte offset after exhausting the source.
///
/// If we've moved past the target line, or if we're on the target line but the
/// column exceeded the line length, return the last byte position on the target
/// line. Otherwise, return the current byte position.
#[inline]
fn clamp_final_offset(
    current: LineColPosition,
    target: LineColPosition,
    last_byte_on_target_line: usize,
    current_byte: usize,
) -> usize {
    if current.line > target.line {
        last_byte_on_target_line
    } else if current.line == target.line && current.col < target.col {
        // Column exceeded line length, clamp to end of line
        last_byte_on_target_line
    } else {
        current_byte
    }
}

/// Convert an LSP Position to a byte offset in the source text.
///
/// This is the inverse of [`byte_offset_to_position`]. It scans the source text
/// to find the byte offset corresponding to the given line and character position.
///
/// The LSP specification defines character positions as UTF-16 code unit offsets.
/// Characters outside the BMP (code points > 0xFFFF) require two UTF-16 code units
/// (a surrogate pair), so they contribute 2 to the column count, not 1.
///
/// If the character position exceeds the line length, this function clamps to the
/// end of the line (just before the newline character, or the end of file for the
/// last line).
///
/// # Arguments
///
/// * `source` - The full source text
/// * `position` - The LSP position (0-based line and character)
///
/// # Examples
///
/// ```
/// use lsp_types::Position;
/// use rstest_bdd_server::handlers::util::lsp_position_to_byte_offset;
///
/// let source = "Feature: demo\n  Scenario: s\n    Given a step\n";
/// // Line 2, column 4 is where "Given" starts
/// let offset = lsp_position_to_byte_offset(source, Position::new(2, 4));
/// assert_eq!(offset, 32);
/// ```
#[must_use]
pub fn lsp_position_to_byte_offset(source: &str, position: Position) -> usize {
    let target_line = position.line;
    let target_col = position.character;

    let mut current_line = 0u32;
    let mut current_col = 0u32;
    let mut current_byte = 0usize;
    // Track the byte offset of the last character on the target line (for clamping)
    let mut last_byte_on_target_line = 0usize;

    for ch in source.chars() {
        // Check if we've reached the target position
        if current_line == target_line && current_col >= target_col {
            break;
        }
        // If we've moved past the target line, clamp to the end of that line
        if current_line > target_line {
            return last_byte_on_target_line;
        }

        // Track the last byte position on the target line (before the newline)
        if current_line == target_line {
            last_byte_on_target_line = current_byte;
        }

        current_byte += ch.len_utf8();

        if ch == '\n' {
            current_line += 1;
            current_col = 0;
        } else {
            current_col += utf16_code_units(ch);
            // Update last byte position after processing non-newline character
            if current_line == target_line {
                last_byte_on_target_line = current_byte;
            }
        }
    }

    clamp_final_offset(
        LineColPosition {
            line: current_line,
            col: current_col,
        },
        LineColPosition {
            line: target_line,
            col: target_col,
        },
        last_byte_on_target_line,
        current_byte,
    )
}

/// Convert a byte column offset to UTF-16 code units for a single line.
///
/// Given a 0-based line number and a byte offset within that line, returns the
/// equivalent UTF-16 code unit column position. This is useful for converting
/// `syn` span columns (which are byte offsets) to LSP positions.
///
/// # Arguments
///
/// * `source` - The full source text
/// * `line_0` - The 0-based line number
/// * `byte_col` - The byte offset within the line
///
/// # Examples
///
/// ```
/// use rstest_bdd_server::handlers::util::byte_col_to_utf16_col;
///
/// // ASCII: byte offset equals UTF-16 column
/// let source = "fn foo() {}";
/// assert_eq!(byte_col_to_utf16_col(source, 0, 3), 3);
///
/// // Non-ASCII: "é" is 2 bytes but 1 UTF-16 code unit
/// let source = "#[given(\"café\")]";
/// // At byte 14 (after "café" ends), UTF-16 column is 13 (one less than bytes)
/// assert_eq!(byte_col_to_utf16_col(source, 0, 14), 13);
/// ```
#[must_use]
pub fn byte_col_to_utf16_col(source: &str, line_0: usize, byte_col: usize) -> u32 {
    let line_text = source.lines().nth(line_0).unwrap_or("");

    // Count UTF-16 code units for characters whose byte position is before byte_col
    line_text
        .char_indices()
        .take_while(|(byte_pos, _)| *byte_pos < byte_col)
        .map(|(_, ch)| utf16_code_units(ch))
        .sum::<u32>()
}

/// Convert a byte offset to an LSP Position (0-based line and character).
///
/// The LSP specification defines character positions as UTF-16 code unit offsets.
/// Characters outside the BMP (code points > 0xFFFF) require two UTF-16 code units
/// (a surrogate pair), so they contribute 2 to the column count, not 1.
fn byte_offset_to_position(source: &str, byte_offset: usize) -> Position {
    let mut line = 0u32;
    let mut col = 0u32;
    let mut current_byte = 0usize;

    for ch in source.chars() {
        if current_byte >= byte_offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += utf16_code_units(ch);
        }
        current_byte += ch.len_utf8();
    }

    Position::new(line, col)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_offset_to_position_first_line() {
        let source = "Feature: demo\n  Scenario: s\n";
        // "Feature" starts at byte 0
        let pos = byte_offset_to_position(source, 0);
        assert_eq!(pos.line, 0);
        assert_eq!(pos.character, 0);

        // "demo" starts at byte 9
        let pos = byte_offset_to_position(source, 9);
        assert_eq!(pos.line, 0);
        assert_eq!(pos.character, 9);
    }

    #[test]
    fn byte_offset_to_position_second_line() {
        let source = "Feature: demo\n  Scenario: s\n";
        // Second line starts at byte 14 (after "Feature: demo\n")
        let pos = byte_offset_to_position(source, 14);
        assert_eq!(pos.line, 1);
        assert_eq!(pos.character, 0);

        // "Scenario" starts at byte 16 (after two spaces)
        let pos = byte_offset_to_position(source, 16);
        assert_eq!(pos.line, 1);
        assert_eq!(pos.character, 2);
    }

    #[test]
    fn gherkin_span_to_lsp_range_single_line() {
        let source = "Feature: demo\n  Scenario: s\n    Given a step\n";
        // "Given a step" is on line 2 (0-indexed), starting at column 4
        let span = Span { start: 32, end: 44 };
        let range = gherkin_span_to_lsp_range(source, span);
        assert_eq!(range.start.line, 2);
        assert_eq!(range.start.character, 4);
        assert_eq!(range.end.line, 2);
        assert_eq!(range.end.character, 16);
    }

    #[test]
    fn handles_empty_source() {
        let source = "";
        let span = Span { start: 0, end: 0 };
        let range = gherkin_span_to_lsp_range(source, span);
        assert_eq!(range.start.line, 0);
        assert_eq!(range.start.character, 0);
        assert_eq!(range.end.line, 0);
        assert_eq!(range.end.character, 0);
    }

    #[test]
    fn byte_offset_to_position_counts_utf16_code_units() {
        // Emoji U+1F600 (😀) is outside the BMP and requires 2 UTF-16 code units
        let source = "hello😀";
        let pos = byte_offset_to_position(source, source.len());
        // 5 ASCII chars (1 UTF-16 code unit each) + 1 emoji (2 UTF-16 code units) = 7
        assert_eq!(pos.character, 7);
    }

    #[test]
    fn byte_offset_to_position_handles_mixed_characters() {
        // Mix of ASCII, BMP (é = U+00E9), and non-BMP (🎉 = U+1F389)
        let source = "café🎉";
        let pos = byte_offset_to_position(source, source.len());
        // 'c' (1) + 'a' (1) + 'f' (1) + 'é' (1, BMP) + '🎉' (2, non-BMP) = 6
        assert_eq!(pos.character, 6);
    }

    #[test]
    fn lsp_position_to_byte_offset_first_line() {
        let source = "Feature: demo\n  Scenario: s\n";
        // Position (0, 0) is byte 0
        let offset = lsp_position_to_byte_offset(source, Position::new(0, 0));
        assert_eq!(offset, 0);

        // Position (0, 9) is byte 9 ("demo" starts here)
        let offset = lsp_position_to_byte_offset(source, Position::new(0, 9));
        assert_eq!(offset, 9);
    }

    #[test]
    fn lsp_position_to_byte_offset_second_line() {
        let source = "Feature: demo\n  Scenario: s\n";
        // Second line starts at byte 14 (after "Feature: demo\n")
        let offset = lsp_position_to_byte_offset(source, Position::new(1, 0));
        assert_eq!(offset, 14);

        // "Scenario" starts at byte 16 (after two spaces, column 2)
        let offset = lsp_position_to_byte_offset(source, Position::new(1, 2));
        assert_eq!(offset, 16);
    }

    #[test]
    fn lsp_position_to_byte_offset_third_line() {
        let source = "Feature: demo\n  Scenario: s\n    Given a step\n";
        // "Given" on line 2 (0-indexed), column 4, starts at byte 32
        let offset = lsp_position_to_byte_offset(source, Position::new(2, 4));
        assert_eq!(offset, 32);
    }

    #[test]
    fn lsp_position_to_byte_offset_handles_empty_source() {
        let source = "";
        let offset = lsp_position_to_byte_offset(source, Position::new(0, 0));
        assert_eq!(offset, 0);
    }

    #[test]
    fn lsp_position_to_byte_offset_handles_non_bmp_characters() {
        // Emoji U+1F600 (😀) is outside the BMP and requires 2 UTF-16 code units
        let source = "hello😀world";
        // After "hello" (5 chars) + emoji (2 UTF-16 units) = column 7
        // "world" starts at byte 5 + 4 (emoji UTF-8) = 9
        let offset = lsp_position_to_byte_offset(source, Position::new(0, 7));
        assert_eq!(offset, 9);
    }

    #[test]
    fn lsp_position_to_byte_offset_roundtrip() {
        let source = "Feature: demo\n  Scenario: s\n    Given a step\n";
        // Test roundtrip: byte -> position -> byte
        for byte_offset in [0, 9, 14, 16, 32, 44] {
            let pos = byte_offset_to_position(source, byte_offset);
            let recovered = lsp_position_to_byte_offset(source, pos);
            assert_eq!(
                recovered, byte_offset,
                "roundtrip failed for offset {byte_offset}"
            );
        }
    }

    #[test]
    fn lsp_position_to_byte_offset_clamps_to_end_of_line() {
        let source = "abc\ndef\n";
        // Request column 100 on line 0 - should clamp to end of "abc" (byte 3)
        let offset = lsp_position_to_byte_offset(source, Position::new(0, 100));
        assert_eq!(offset, 3, "should clamp to end of line 0");

        // Request column 100 on line 1 - should clamp to end of "def" (byte 7)
        let offset = lsp_position_to_byte_offset(source, Position::new(1, 100));
        assert_eq!(offset, 7, "should clamp to end of line 1");
    }

    #[test]
    fn lsp_position_to_byte_offset_clamps_to_eof_on_final_line() {
        let source = "abc\ndef"; // No trailing newline
        // Request column 100 on line 1 - should clamp to end of file (byte 7)
        let offset = lsp_position_to_byte_offset(source, Position::new(1, 100));
        assert_eq!(offset, 7, "should clamp to end of file");
    }
}
