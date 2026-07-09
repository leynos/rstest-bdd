//! Shared utilities for LSP position handling.
//!
//! This module provides fundamental UTF-16 conversion utilities used by both
//! the indexing and handler modules. By centralizing these helpers here, we
//! avoid inverted dependencies between modules.

/// Calculate UTF-16 code units for a character.
///
/// BMP characters (code points ≤ 0xFFFF) use 1 UTF-16 code unit.
/// Non-BMP characters (code points > 0xFFFF) use 2 UTF-16 code units (surrogate pair).
///
/// # Examples
///
/// ```
/// use rstest_bdd_server::util::utf16_code_units;
///
/// // ASCII and BMP characters use 1 code unit
/// assert_eq!(utf16_code_units('a'), 1);
/// assert_eq!(utf16_code_units('é'), 1); // U+00E9, BMP
///
/// // Non-BMP characters (emojis) use 2 code units
/// assert_eq!(utf16_code_units('😀'), 2); // U+1F600, non-BMP
/// assert_eq!(utf16_code_units('🎉'), 2); // U+1F389, non-BMP
/// ```
#[inline]
#[must_use]
pub fn utf16_code_units(ch: char) -> u32 {
    if u32::from(ch) <= 0xFFFF { 1 } else { 2 }
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
/// # Returns
///
/// The equivalent UTF-16 code unit column position. Returns 0 if the line
/// is not found or if `byte_col` is 0.
///
/// # Examples
///
/// ```
/// use rstest_bdd_server::util::byte_col_to_utf16_col;
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

#[cfg(test)]
mod tests {
    //! Unit tests for shared utility helpers.

    use rstest::rstest;

    use super::*;

    // --- utf16_code_units tests ---

    #[rstest]
    #[case('a', 1)]
    #[case('Z', 1)]
    #[case('0', 1)]
    #[case(' ', 1)]
    #[case('\n', 1)]
    fn utf16_code_units_ascii_characters(#[case] ch: char, #[case] expected: u32) {
        assert_eq!(utf16_code_units(ch), expected);
    }

    #[rstest]
    #[case('é', 1)] // Latin Extended (U+00E9)
    #[case('α', 1)] // Greek (U+03B1)
    #[case('日', 1)] // CJK (U+65E5)
    #[case('ع', 1)] // Arabic (U+0639)
    fn utf16_code_units_bmp_non_ascii(#[case] ch: char, #[case] expected: u32) {
        assert_eq!(utf16_code_units(ch), expected);
    }

    #[rstest]
    #[case('😀', 2)] // Emoji (U+1F600)
    #[case('🎉', 2)] // Emoji (U+1F389)
    #[case('🦀', 2)] // Emoji (U+1F980, Rust crab!)
    fn utf16_code_units_non_bmp_emojis(#[case] ch: char, #[case] expected: u32) {
        assert_eq!(utf16_code_units(ch), expected);
    }

    #[rstest]
    #[case('\u{FFFF}', 1)] // Last BMP character (U+FFFF)
    #[case('\u{10000}', 2)] // First non-BMP character (U+10000)
    fn utf16_code_units_boundary_cases(#[case] ch: char, #[case] expected: u32) {
        assert_eq!(utf16_code_units(ch), expected);
    }

    // --- byte_col_to_utf16_col tests ---

    #[test]
    fn byte_col_ascii_only_line() {
        let source = "fn foo() {}";
        // For ASCII, byte offset equals UTF-16 column
        assert_eq!(byte_col_to_utf16_col(source, 0, 0), 0);
        assert_eq!(byte_col_to_utf16_col(source, 0, 3), 3);
        assert_eq!(byte_col_to_utf16_col(source, 0, 11), 11);
    }

    #[test]
    fn byte_col_multi_byte_utf8_bmp() {
        // "é" is 2 bytes in UTF-8 but 1 UTF-16 code unit
        // String: #[given("café")] = 17 bytes, 16 UTF-16 units
        // "é" starts at byte 12 (2 bytes), ends at byte 14
        let source = "#[given(\"café\")]";
        // Before "é": #[given("caf = 12 bytes, 12 UTF-16 units
        assert_eq!(byte_col_to_utf16_col(source, 0, 12), 12);
        // After "é" (byte 14), UTF-16 column is 13 (12 + 1 for é)
        assert_eq!(byte_col_to_utf16_col(source, 0, 14), 13);
        // At end (byte 17 = after closing ]), UTF-16 is 16
        assert_eq!(byte_col_to_utf16_col(source, 0, 17), 16);
    }

    #[test]
    fn byte_col_non_bmp_emoji() {
        // 😀 is 4 bytes in UTF-8 but 2 UTF-16 code units
        let source = "hello😀world";
        // "hello" = 5 bytes, 5 UTF-16 units
        assert_eq!(byte_col_to_utf16_col(source, 0, 5), 5);
        // After emoji (byte 9), UTF-16 column is 7 (5 + 2)
        assert_eq!(byte_col_to_utf16_col(source, 0, 9), 7);
        // "world" ends at byte 14, UTF-16 column is 12 (7 + 5)
        assert_eq!(byte_col_to_utf16_col(source, 0, 14), 12);
    }

    /// Mix of ASCII, BMP non-ASCII, and non-BMP.
    /// "café🎉" = c(1) + a(1) + f(1) + é(2) + 🎉(4) = 9 bytes
    #[rstest]
    #[case(0, 0)]
    #[case(3, 3)] // After "caf"
    #[case(5, 4)] // After "café"
    #[case(9, 6)] // After "café🎉"
    fn byte_col_mixed_characters(#[case] byte_col: usize, #[case] expected: u32) {
        let source = "café🎉";
        assert_eq!(byte_col_to_utf16_col(source, 0, byte_col), expected);
    }

    /// Multiline source: "line1\nlinéé2\nline3"
    /// Line 0: "line1" (ASCII only)
    /// Line 1: "linéé2" - "lin" = 3 bytes/units, then "é" = 2 bytes each
    /// Line 2: "line3" (ASCII only)
    #[rstest]
    #[case(0, 3, 3)] // Line 0: after "lin"
    #[case(1, 3, 3)] // Line 1: after "lin"
    #[case(1, 5, 4)] // Line 1: after first "é"
    #[case(1, 7, 5)] // Line 1: after second "é"
    #[case(2, 5, 5)] // Line 2: after "line3"
    fn byte_col_multiline_source(
        #[case] line: usize,
        #[case] byte_col: usize,
        #[case] expected: u32,
    ) {
        let source = "line1\nlinéé2\nline3";
        assert_eq!(byte_col_to_utf16_col(source, line, byte_col), expected);
    }

    #[test]
    fn byte_col_empty_source() {
        let source = "";
        assert_eq!(byte_col_to_utf16_col(source, 0, 0), 0);
        assert_eq!(byte_col_to_utf16_col(source, 0, 10), 0);
    }

    #[test]
    fn byte_col_line_not_found() {
        let source = "single line";
        // Line 1 doesn't exist, should return 0
        assert_eq!(byte_col_to_utf16_col(source, 1, 5), 0);
        assert_eq!(byte_col_to_utf16_col(source, 99, 5), 0);
    }

    #[test]
    fn byte_col_beyond_line_length() {
        let source = "short";
        // byte_col beyond line length should return the full line's UTF-16 length
        assert_eq!(byte_col_to_utf16_col(source, 0, 100), 5);
    }

    /// CJK characters are BMP (1 UTF-16 unit) but 3 bytes in UTF-8.
    /// "日本語" = 9 bytes, 3 UTF-16 units
    #[rstest]
    #[case(0, 0)]
    #[case(3, 1)] // After first char
    #[case(6, 2)] // After second char
    #[case(9, 3)] // After third char
    fn byte_col_cjk_characters(#[case] byte_col: usize, #[case] expected: u32) {
        let source = "日本語";
        assert_eq!(byte_col_to_utf16_col(source, 0, byte_col), expected);
    }

    #[test]
    fn byte_col_realistic_step_attribute() {
        // Realistic step attribute with non-ASCII in the pattern
        // String: #[given("a user named José")] = 30 bytes, 29 UTF-16 units
        // "é" starts at byte 25 (2 bytes), ends at byte 27
        let source = "#[given(\"a user named José\")]";
        // #[given("a user named Jos = 25 bytes, 25 UTF-16 units
        assert_eq!(byte_col_to_utf16_col(source, 0, 25), 25);
        // After "é" (byte 27), UTF-16 column is 26 (25 + 1 for é)
        assert_eq!(byte_col_to_utf16_col(source, 0, 27), 26);
        // End of attribute (byte 30), UTF-16 column is 29
        assert_eq!(byte_col_to_utf16_col(source, 0, 30), 29);
    }
}
