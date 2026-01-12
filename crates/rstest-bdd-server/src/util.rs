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
    use super::*;

    // --- utf16_code_units tests ---

    #[test]
    fn utf16_code_units_ascii_characters() {
        assert_eq!(utf16_code_units('a'), 1);
        assert_eq!(utf16_code_units('Z'), 1);
        assert_eq!(utf16_code_units('0'), 1);
        assert_eq!(utf16_code_units(' '), 1);
        assert_eq!(utf16_code_units('\n'), 1);
    }

    #[test]
    fn utf16_code_units_bmp_non_ascii() {
        // Latin Extended (é = U+00E9)
        assert_eq!(utf16_code_units('é'), 1);
        // Greek (α = U+03B1)
        assert_eq!(utf16_code_units('α'), 1);
        // CJK (日 = U+65E5)
        assert_eq!(utf16_code_units('日'), 1);
        // Arabic (ع = U+0639)
        assert_eq!(utf16_code_units('ع'), 1);
    }

    #[test]
    fn utf16_code_units_non_bmp_emojis() {
        // Emoji (😀 = U+1F600)
        assert_eq!(utf16_code_units('😀'), 2);
        // Emoji (🎉 = U+1F389)
        assert_eq!(utf16_code_units('🎉'), 2);
        // Emoji (🦀 = U+1F980, Rust crab!)
        assert_eq!(utf16_code_units('🦀'), 2);
    }

    #[test]
    fn utf16_code_units_boundary_cases() {
        // Last BMP character (U+FFFF)
        assert_eq!(utf16_code_units('\u{FFFF}'), 1);
        // First non-BMP character (U+10000)
        assert_eq!(utf16_code_units('\u{10000}'), 2);
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
        let source = "#[given(\"café\")]";
        // Positions before "é": #[given("caf = 11 bytes, 11 UTF-16 units
        assert_eq!(byte_col_to_utf16_col(source, 0, 11), 11);
        // After "é" (byte 13), UTF-16 column is 12
        assert_eq!(byte_col_to_utf16_col(source, 0, 13), 12);
        // At end (byte 16 = after closing ]), UTF-16 is 14
        assert_eq!(byte_col_to_utf16_col(source, 0, 16), 14);
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

    #[test]
    fn byte_col_mixed_characters() {
        // Mix of ASCII, BMP non-ASCII, and non-BMP
        // "café🎉" = c(1) + a(1) + f(1) + é(2) + 🎉(4) = 9 bytes
        let source = "café🎉";
        assert_eq!(byte_col_to_utf16_col(source, 0, 0), 0);
        assert_eq!(byte_col_to_utf16_col(source, 0, 3), 3); // After "caf"
        assert_eq!(byte_col_to_utf16_col(source, 0, 5), 4); // After "café"
        assert_eq!(byte_col_to_utf16_col(source, 0, 9), 6); // After "café🎉"
    }

    #[test]
    fn byte_col_multiline_source() {
        let source = "line1\nlinéé2\nline3";
        // Line 0: "line1" (ASCII only)
        assert_eq!(byte_col_to_utf16_col(source, 0, 3), 3);
        // Line 1: "linéé2" - "lin" = 3 bytes/units, then "é" = 2 bytes each
        assert_eq!(byte_col_to_utf16_col(source, 1, 3), 3); // After "lin"
        assert_eq!(byte_col_to_utf16_col(source, 1, 5), 4); // After first "é"
        assert_eq!(byte_col_to_utf16_col(source, 1, 7), 5); // After second "é"
        // Line 2: "line3" (ASCII only)
        assert_eq!(byte_col_to_utf16_col(source, 2, 5), 5);
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

    #[test]
    fn byte_col_cjk_characters() {
        // CJK characters are BMP (1 UTF-16 unit) but 3 bytes in UTF-8
        // "日本語" = 9 bytes, 3 UTF-16 units
        let source = "日本語";
        assert_eq!(byte_col_to_utf16_col(source, 0, 0), 0);
        assert_eq!(byte_col_to_utf16_col(source, 0, 3), 1); // After first char
        assert_eq!(byte_col_to_utf16_col(source, 0, 6), 2); // After second char
        assert_eq!(byte_col_to_utf16_col(source, 0, 9), 3); // After third char
    }

    #[test]
    fn byte_col_realistic_step_attribute() {
        // Realistic step attribute with non-ASCII in the pattern
        let source = "#[given(\"a user named José\")]";
        // #[given("a user named Jos = 24 bytes, 24 UTF-16 units
        assert_eq!(byte_col_to_utf16_col(source, 0, 24), 24);
        // After "é" (byte 26), UTF-16 column is 25
        assert_eq!(byte_col_to_utf16_col(source, 0, 26), 25);
        // End of attribute (byte 29), UTF-16 column is 27
        assert_eq!(byte_col_to_utf16_col(source, 0, 29), 27);
    }
}
