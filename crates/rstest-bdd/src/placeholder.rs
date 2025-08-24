//! Placeholder extraction and pattern-to-regex compilation.
//! This module implements `extract_placeholders` and the internal single-pass
//! scanner that converts `{name[:type]}` segments into a safe regular
//! expression. Helpers are `pub(crate)` to support internal tests.

use crate::pattern::StepPattern;
use crate::types::{PlaceholderError, StepText};
use regex::Regex;

/// Extract placeholder values from a step string using a pattern.
/// See crate-level docs for the accepted syntax and error cases.
///
/// # Errors
/// Returns [`PlaceholderError::InvalidPattern`] if the pattern cannot be
/// compiled, [`PlaceholderError::Uncompiled`] if the pattern was not compiled
/// before use (guard), and [`PlaceholderError::PatternMismatch`] when the text
/// does not satisfy the pattern.
pub fn extract_placeholders(
    pattern: &StepPattern,
    text: StepText<'_>,
) -> Result<Vec<String>, PlaceholderError> {
    pattern
        .compile()
        .map_err(|e| PlaceholderError::InvalidPattern(e.to_string()))?;
    let re = pattern.try_regex().ok_or(PlaceholderError::Uncompiled)?;
    extract_captured_values(re, text.as_str()).ok_or(PlaceholderError::PatternMismatch)
}

pub(crate) fn get_type_pattern(type_hint: Option<&str>) -> &'static str {
    match type_hint {
        Some("u8" | "u16" | "u32" | "u64" | "u128" | "usize") => r"\d+",
        Some("i8" | "i16" | "i32" | "i64" | "i128" | "isize") => r"[+-]?\d+",
        Some("f32" | "f64") => {
            r"(?i:(?:[+-]?(?:\d+\.\d*|\.\d+|\d+)(?:[eE][+-]?\d+)?|nan|inf|infinity))"
        }
        _ => r".+?",
    }
}

pub(crate) fn extract_captured_values(re: &Regex, text: &str) -> Option<Vec<String>> {
    let caps = re.captures(text)?;
    let mut values = Vec::new();
    for i in 1..caps.len() {
        values.push(caps[i].to_string());
    }
    Some(values)
}

// Scanner state and helpers (pub(crate) for internal tests)
pub(crate) struct RegexBuilder<'a> {
    pub(crate) pattern: &'a str,
    pub(crate) bytes: &'a [u8],
    pub(crate) position: usize,
    pub(crate) output: String,
}

impl<'a> RegexBuilder<'a> {
    pub(crate) fn new(pattern: &'a str) -> Self {
        let mut output = String::with_capacity(pattern.len().saturating_mul(2) + 2);
        output.push('^');
        Self {
            pattern,
            bytes: pattern.as_bytes(),
            position: 0,
            output,
        }
    }
    #[inline]
    pub(crate) fn has_more(&self) -> bool {
        self.position < self.bytes.len()
    }
    #[inline]
    pub(crate) fn advance(&mut self, n: usize) {
        self.position = self.position.saturating_add(n);
    }
    #[inline]
    pub(crate) fn push_literal_byte(&mut self, b: u8) {
        self.output
            .push_str(&regex::escape(&(b as char).to_string()));
    }
    #[inline]
    pub(crate) fn push_literal_brace(&mut self, brace: u8) {
        self.push_literal_byte(brace);
    }
    #[inline]
    pub(crate) fn push_capture_for_type(&mut self, ty: Option<&str>) {
        self.output.push('(');
        self.output.push_str(get_type_pattern(ty));
        self.output.push(')');
    }
}

#[inline]
pub(crate) fn is_escaped_brace(bytes: &[u8], pos: usize) -> bool {
    matches!(bytes.get(pos), Some(b'\\')) && matches!(bytes.get(pos + 1), Some(b'{' | b'}'))
}

#[inline]
pub(crate) fn is_double_brace(bytes: &[u8], pos: usize) -> bool {
    let first = match bytes.get(pos) {
        Some(b @ (b'{' | b'}')) => *b,
        _ => return false,
    };
    matches!(bytes.get(pos + 1), Some(b) if *b == first)
}

#[inline]
pub(crate) fn is_placeholder_start(bytes: &[u8], pos: usize) -> bool {
    matches!(bytes.get(pos), Some(b'{'))
        && matches!(bytes.get(pos + 1), Some(b) if (*b as char).is_ascii_alphabetic() || *b == b'_')
}

#[inline]
pub(crate) fn is_empty_type_hint(state: &RegexBuilder<'_>, name_end: usize) -> bool {
    if !matches!(state.bytes.get(name_end), Some(b':')) {
        return false;
    }
    let mut i = name_end + 1;
    while let Some(&b) = state.bytes.get(i) {
        if b == b'}' {
            return true;
        }
        if !(b as char).is_ascii_whitespace() {
            return false;
        }
        i += 1;
    }
    false
}

pub(crate) fn parse_escaped_brace(state: &mut RegexBuilder<'_>) {
    #[expect(clippy::indexing_slicing, reason = "predicate ensured bound")]
    let ch = state.bytes[state.position + 1];
    state.push_literal_brace(ch);
    state.advance(2);
}

/// Parses a backslash escape that is not an escaped brace and emits the next
/// byte as a literal.
///
/// Callers must ensure the current byte is `\` and the following byte is not a
/// brace; `parse_escaped_brace` handles those cases. Trailing backslashes
/// delegate to [`try_parse_common_sequences`], which emits a literal backslash.
///
/// # Examples
/// ```
/// # use crate::placeholder::{parse_escape_sequence, RegexBuilder};
/// let mut st = RegexBuilder::new(r"\x");
/// parse_escape_sequence(&mut st);
/// assert_eq!(st.output, "x");
/// ```
pub(crate) fn parse_escape_sequence(state: &mut RegexBuilder<'_>) {
    debug_assert!(matches!(state.bytes.get(state.position), Some(b'\\')));
    debug_assert!(!is_escaped_brace(state.bytes, state.position));
    debug_assert!(state.bytes.get(state.position + 1).is_some());
    if let Some(&next) = state.bytes.get(state.position + 1) {
        state.push_literal_byte(next);
        state.advance(2);
    } else if try_parse_common_sequences(state) {
        // Trailing backslash handled by common sequences.
    }
}

pub(crate) fn parse_double_brace(state: &mut RegexBuilder<'_>) {
    #[expect(clippy::indexing_slicing, reason = "predicate ensured bound")]
    let brace = state.bytes[state.position];
    state.push_literal_brace(brace);
    state.advance(2);
}

pub(crate) fn parse_literal(state: &mut RegexBuilder<'_>) {
    #[expect(clippy::indexing_slicing, reason = "caller ensured bound")]
    let ch = state.bytes[state.position];
    state.push_literal_byte(ch);
    state.advance(1);
}

pub(crate) fn parse_placeholder_name(state: &RegexBuilder<'_>, start: usize) -> (usize, String) {
    let mut i = start + 1; // skip '{'
    let mut name = String::new();
    while let Some(&b) = state.bytes.get(i) {
        if (b as char).is_ascii_alphanumeric() || b == b'_' {
            name.push(b as char);
            i += 1;
        } else {
            break;
        }
    }
    (i, name)
}

pub(crate) fn parse_type_hint(state: &RegexBuilder<'_>, start: usize) -> (usize, Option<String>) {
    let mut i = start;
    if !matches!(state.bytes.get(i), Some(b':')) {
        return (i, None);
    }
    i += 1;
    let ty_start = i;
    let mut nest = 0usize;
    while let Some(&b) = state.bytes.get(i) {
        match b {
            b'{' => {
                nest += 1;
                i += 1;
            }
            b'}' => {
                if nest == 0 {
                    break;
                }
                nest -= 1;
                i += 1;
            }
            _ => i += 1,
        }
    }
    #[expect(clippy::string_slice, reason = "ASCII region delimited by braces")]
    let ty = state.pattern[ty_start..i].trim().to_string();
    if ty.is_empty() {
        return (start, None);
    }
    (i, Some(ty))
}

pub(crate) fn parse_placeholder(state: &mut RegexBuilder<'_>) -> Result<(), regex::Error> {
    let start = state.position;
    let (name_end, _name) = parse_placeholder_name(state, start + 1);
    if let Some(b) = state.bytes.get(name_end) {
        if (*b as char).is_ascii_whitespace() {
            let mut ws = name_end;
            while let Some(bw) = state.bytes.get(ws) {
                if !(*bw as char).is_ascii_whitespace() {
                    break;
                }
                ws += 1;
            }
            if matches!(state.bytes.get(ws), Some(b':')) {
                return Err(regex::Error::Syntax(
                    "invalid placeholder in step pattern".to_string(),
                ));
            }
        }
    }
    if is_empty_type_hint(state, name_end) {
        return Err(regex::Error::Syntax(
            "invalid placeholder in step pattern".to_string(),
        ));
    }
    let (mut after, ty_opt) = parse_type_hint(state, name_end);
    if ty_opt.is_none() {
        // No explicit type hint; scan to matching '}' allowing nested.
        let mut k = name_end;
        let mut nest = 0usize;
        while let Some(&b) = state.bytes.get(k) {
            match b {
                b'{' => {
                    nest += 1;
                    k += 1;
                }
                b'}' => {
                    if nest == 0 {
                        break;
                    }
                    nest -= 1;
                    k += 1;
                }
                _ => k += 1,
            }
        }
        after = k;
    }
    if !matches!(state.bytes.get(after), Some(b'}')) {
        return Err(regex::Error::Syntax(
            "unbalanced braces in step pattern".to_string(),
        ));
    }
    state.push_capture_for_type(ty_opt.as_deref());
    if ty_opt.as_ref().is_some_and(|t| t.contains('{')) {
        state.output.push_str(r"\}");
    }
    after += 1; // skip closing brace
    state.position = after;
    Ok(())
}

/// Parses sequences common to all contexts: doubled braces, escaped braces, or
/// backslash escapes. Returns `true` if a recognised sequence was consumed.
#[inline]
pub(crate) fn try_parse_common_sequences(st: &mut RegexBuilder<'_>) -> bool {
    if is_double_brace(st.bytes, st.position) {
        parse_double_brace(st);
        true
    } else if is_escaped_brace(st.bytes, st.position) {
        parse_escaped_brace(st);
        true
    } else if matches!(st.bytes.get(st.position), Some(b'\\')) {
        if st.bytes.get(st.position + 1).is_some() {
            parse_escape_sequence(st);
        } else {
            // Trailing backslash is treated literally.
            st.push_literal_byte(b'\\');
            st.advance(1);
        }
        true
    } else {
        false
    }
}

/// Emits the current byte as a literal while inside stray-depth
/// (`st.stray_depth > 0`), adjusting for nested braces as needed.
///
/// This path is reached only when `stray_depth` is non-zero.
#[inline]
pub(crate) fn parse_stray_character(st: &mut RegexBuilder<'_>) {
    #[expect(clippy::indexing_slicing, reason = "bounds checked by caller")]
    let ch = st.bytes[st.position];
    match ch {
        b'{' => st.stray_depth = st.stray_depth.saturating_add(1),
        b'}' => st.stray_depth = st.stray_depth.saturating_sub(1),
        _ => {}
    }
    st.push_literal_byte(ch);
    st.advance(1);
}

<<<<<<< HEAD
pub(crate) fn parse_context_specific(st: &mut RegexBuilder<'_>) -> Result<(), regex::Error> {
||||||| parent of 5e17738 (Document escape handling and expand tests)
pub(crate) fn parse_context_specific(st: &mut RegexBuilder<'_>) {
=======
/// Dispatches context-specific parsing after common sequences.
///
/// When scanning stray text (inside unmatched braces), it emits the next
/// character as a literal. Otherwise it parses a placeholder start or a simple
/// literal.
#[inline]
pub(crate) fn parse_context_specific(st: &mut RegexBuilder<'_>) {
>>>>>>> 5e17738 (Document escape handling and expand tests)
    if st.stray_depth > 0 {
        parse_stray_character(st);
        Ok(())
    } else if is_placeholder_start(st.bytes, st.position) {
        parse_placeholder(st)
    } else {
        parse_literal(st);
        Ok(())
    }
}

pub(crate) fn build_regex_from_pattern(pat: &str) -> Result<String, regex::Error> {
    let mut st = RegexBuilder::new(pat);
    while st.has_more() {
        if !try_parse_common_sequences(&mut st) {
            parse_context_specific(&mut st)?;
        }
    }
    st.output.push('$');
    Ok(st.output)
}
