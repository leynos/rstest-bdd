//! Unit tests for handler utility helpers.

use super::*;

#[test]
fn byte_offset_to_position_first_line() {
    let source = "Feature: demo\n  Scenario: s\n";
    // "Feature" starts at byte 0
    let pos = byte_offset_to_position(source, 0);
    assert_eq!(pos.line, 0);
    assert_eq!(pos.character, 0);

    // "demo" starts at byte 9
    let demo_pos = byte_offset_to_position(source, 9);
    assert_eq!(demo_pos.line, 0);
    assert_eq!(demo_pos.character, 9);
}

#[test]
fn byte_offset_to_position_second_line() {
    let source = "Feature: demo\n  Scenario: s\n";
    // Second line starts at byte 14 (after "Feature: demo\n")
    let pos = byte_offset_to_position(source, 14);
    assert_eq!(pos.line, 1);
    assert_eq!(pos.character, 0);

    // "Scenario" starts at byte 16 (after two spaces)
    let scenario_pos = byte_offset_to_position(source, 16);
    assert_eq!(scenario_pos.line, 1);
    assert_eq!(scenario_pos.character, 2);
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
    let demo_offset = lsp_position_to_byte_offset(source, Position::new(0, 9));
    assert_eq!(demo_offset, 9);
}

#[test]
fn lsp_position_to_byte_offset_second_line() {
    let source = "Feature: demo\n  Scenario: s\n";
    // Second line starts at byte 14 (after "Feature: demo\n")
    let offset = lsp_position_to_byte_offset(source, Position::new(1, 0));
    assert_eq!(offset, 14);

    // "Scenario" starts at byte 16 (after two spaces, column 2)
    let scenario_offset = lsp_position_to_byte_offset(source, Position::new(1, 2));
    assert_eq!(scenario_offset, 16);
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
    let second_line_offset = lsp_position_to_byte_offset(source, Position::new(1, 100));
    assert_eq!(second_line_offset, 7, "should clamp to end of line 1");
}

#[test]
fn lsp_position_to_byte_offset_clamps_to_eof_on_final_line() {
    let source = "abc\ndef"; // No trailing newline
    // Request column 100 on line 1 - should clamp to end of file (byte 7)
    let offset = lsp_position_to_byte_offset(source, Position::new(1, 100));
    assert_eq!(offset, 7, "should clamp to end of file");
}
