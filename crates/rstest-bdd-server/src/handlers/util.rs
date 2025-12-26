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
            // UTF-16 code units: BMP characters (≤0xFFFF) use 1, non-BMP use 2
            col += if u32::from(ch) <= 0xFFFF { 1 } else { 2 };
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
}
