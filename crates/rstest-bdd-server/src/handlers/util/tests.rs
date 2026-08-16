//! Unit tests for handler utility helpers.

use super::*;
use proptest::prelude::*;
use rstest::{fixture, rstest};

#[fixture]
fn two_line_source() -> &'static str {
    "Feature: demo\n  Scenario: s\n"
}

#[fixture]
fn three_line_source() -> &'static str {
    "Feature: demo\n  Scenario: s\n    Given a step\n"
}

#[rstest]
#[case(0, 0, 0)]
#[case(9, 0, 9)]
#[case(14, 1, 0)]
#[case(16, 1, 2)]
fn byte_offset_to_position_in_two_line_source(
    two_line_source: &str,
    #[case] byte_offset: usize,
    #[case] line: u32,
    #[case] character: u32,
) {
    assert_eq!(
        byte_offset_to_position(two_line_source, byte_offset),
        Position::new(line, character)
    );
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

#[rstest]
#[case(0, 0, 0)]
#[case(0, 9, 9)]
#[case(1, 0, 14)]
#[case(1, 2, 16)]
fn lsp_position_to_byte_offset_in_two_line_source(
    two_line_source: &str,
    #[case] line: u32,
    #[case] character: u32,
    #[case] expected_offset: usize,
) {
    assert_eq!(
        lsp_position_to_byte_offset(two_line_source, Position::new(line, character)),
        expected_offset
    );
}

#[rstest]
#[case(32, 2, 4)]
fn byte_offset_to_position_in_three_line_source(
    three_line_source: &str,
    #[case] byte_offset: usize,
    #[case] line: u32,
    #[case] character: u32,
) {
    assert_eq!(
        byte_offset_to_position(three_line_source, byte_offset),
        Position::new(line, character)
    );
}

#[rstest]
#[case(2, 4, 32)]
fn lsp_position_to_byte_offset_in_three_line_source(
    three_line_source: &str,
    #[case] line: u32,
    #[case] character: u32,
    #[case] expected_offset: usize,
) {
    assert_eq!(
        lsp_position_to_byte_offset(three_line_source, Position::new(line, character)),
        expected_offset
    );
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
    assert_eq!(lsp_position_to_byte_offset(source, Position::new(0, 6)), 9);
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

proptest! {
    #[test]
    fn character_boundaries_roundtrip(source in "[a-zA-Z0-9😀é\\n]{0,40}") {
        for byte_offset in source.char_indices().map(|(offset, _)| offset).chain(std::iter::once(source.len())) {
            let position = byte_offset_to_position(&source, byte_offset);
            prop_assert_eq!(lsp_position_to_byte_offset(&source, position), byte_offset);
        }
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
