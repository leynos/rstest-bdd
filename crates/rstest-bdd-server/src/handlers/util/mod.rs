//! Handler utilities for LSP type conversions and path predicates.
//!
//! This module provides helper functions for converting between `gherkin` types
//! and LSP protocol types, particularly for span and position conversions, and
//! the canonical [`has_extension`] predicate handlers use to distinguish `.rs`
//! from `.feature` paths.
//!
//! Note: Fundamental UTF-16 conversion utilities are in [`crate::util`] to avoid
//! circular dependencies between modules.

use std::path::Path;

use gherkin::Span;
use lsp_types::{Position, Range};

use crate::util::utf16_code_units;

// Re-export for backwards compatibility
pub use crate::util::byte_col_to_utf16_col;

/// Check whether `path` has the file extension `ext`, ignoring ASCII case.
///
/// This is the canonical extension predicate for handler code. Handlers
/// distinguishing `.rs` from `.feature` paths must call this helper rather
/// than reimplementing the check locally; see the developers' guide for the
/// ownership and composition rules.
///
/// `ext` is supplied without a leading dot. The comparison is
/// case-insensitive over ASCII only, matching how file extensions are
/// conventionally compared on case-preserving filesystems. Paths without an
/// extension never match.
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use rstest_bdd_server::handlers::util::has_extension;
///
/// assert!(has_extension(Path::new("steps.rs"), "rs"));
/// assert!(has_extension(Path::new("demo.FEATURE"), "feature"));
/// assert!(!has_extension(Path::new("notes.txt"), "rs"));
/// assert!(!has_extension(Path::new("no_extension"), "rs"));
/// ```
#[must_use]
pub fn has_extension(path: &Path, ext: &str) -> bool {
    path.extension()
        .and_then(std::ffi::OsStr::to_str)
        .is_some_and(|actual| actual.eq_ignore_ascii_case(ext))
}

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
/// let span = Span { start: 32, end: 44 }; // "Given a step"
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
/// This is the inverse of `byte_offset_to_position`. It scans the source text
/// to find the byte offset corresponding to the given line and character position.
///
/// The LSP specification defines character positions as UTF-16 code unit offsets.
/// Characters outside the BMP (code points > 0xFFFF) require two UTF-16 code units
/// (a surrogate pair), so they contribute 2 to the column count, not 1.
/// If a UTF-16 position splits a surrogate pair, it snaps forward to the next
/// character boundary. For example, character 6 in `"hello😀world"` is inside
/// the emoji's surrogate pair and resolves to the start of `world`.
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
mod tests;
