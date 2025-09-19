//! Shared step-pattern parsing utilities for rstest-bdd.
use regex::Regex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceholderErrorInfo {
    pub message: &'static str,
    pub position: usize,
    pub placeholder: Option<String>,
}

impl PlaceholderErrorInfo {
    #[must_use]
    pub fn new(message: &'static str, position: usize, placeholder: Option<String>) -> Self {
        Self {
            message,
            position,
            placeholder,
        }
    }
}

#[derive(Debug)]
pub enum PatternError {
    Placeholder(PlaceholderErrorInfo),
    Regex(regex::Error),
}

#[must_use]
pub fn get_type_pattern(type_hint: Option<&str>) -> &'static str {
    match type_hint {
        Some("u8" | "u16" | "u32" | "u64" | "u128" | "usize") => r"\d+",
        Some("i8" | "i16" | "i32" | "i64" | "i128" | "isize") => r"[+-]?\d+",
        Some("f32" | "f64") => {
            r"(?i:(?:[+-]?(?:\d+\.\d*|\.\d+|\d+)(?:[eE][+-]?\d+)?|nan|inf|infinity))"
        }
        _ => r".+?",
    }
}

#[must_use]
pub fn extract_captured_values(re: &Regex, text: &str) -> Option<Vec<String>> {
    let caps = re.captures(text)?;
    let mut values = Vec::new();
    for i in 1..caps.len() {
        values.push(caps[i].to_string());
    }
    Some(values)
}

pub struct RegexBuilder<'a> {
    pub pattern: &'a str,
    pub bytes: &'a [u8],
    pub position: usize,
    pub output: String,
    pub stray_depth: usize,
}

impl<'a> RegexBuilder<'a> {
    #[must_use]
    pub fn new(pattern: &'a str) -> Self {
        let mut output = String::with_capacity(pattern.len().saturating_mul(2) + 2);
        output.push('^');
        Self {
            pattern,
            bytes: pattern.as_bytes(),
            position: 0,
            output,
            stray_depth: 0,
        }
    }
    #[inline]
    #[must_use]
    pub fn has_more(&self) -> bool {
        self.position < self.bytes.len()
    }
    #[inline]
    pub fn advance(&mut self, n: usize) {
        self.position = self.position.saturating_add(n);
    }
    #[inline]
    pub fn push_literal_byte(&mut self, b: u8) {
        self.output
            .push_str(&regex::escape(&(b as char).to_string()));
    }
    pub fn push_capture_for_type(&mut self, ty: Option<&str>) {
        self.output.push('(');
        self.output.push_str(get_type_pattern(ty));
        self.output.push(')');
    }
}

#[inline]
#[must_use]
pub fn is_escaped_brace(bytes: &[u8], pos: usize) -> bool {
    matches!(bytes.get(pos), Some(b'\\')) && matches!(bytes.get(pos + 1), Some(b'{' | b'}'))
}

#[inline]
#[must_use]
pub fn is_double_brace(bytes: &[u8], pos: usize) -> bool {
    let first = match bytes.get(pos) {
        Some(b @ (b'{' | b'}')) => *b,
        _ => return false,
    };
    matches!(bytes.get(pos + 1), Some(b) if *b == first)
}

#[inline]
#[must_use]
pub fn is_placeholder_start(bytes: &[u8], pos: usize) -> bool {
    matches!(bytes.get(pos), Some(b'{'))
        && matches!(bytes.get(pos + 1), Some(b) if (*b as char).is_ascii_alphabetic() || *b == b'_')
}

pub fn parse_escaped_brace(state: &mut RegexBuilder<'_>) {
    #[expect(clippy::indexing_slicing, reason = "predicate ensured bound")]
    let ch = state.bytes[state.position + 1];
    state.push_literal_byte(ch);
    state.advance(2);
}

pub fn parse_escape_sequence(state: &mut RegexBuilder<'_>) {
    debug_assert!(matches!(state.bytes.get(state.position), Some(b'\\')));
    debug_assert!(!is_escaped_brace(state.bytes, state.position));
    debug_assert!(state.bytes.get(state.position + 1).is_some());
    #[expect(
        clippy::indexing_slicing,
        reason = "preceding debug_assert ensures bound"
    )]
    let next = state.bytes[state.position + 1];
    state.push_literal_byte(next);
    state.advance(2);
}

pub fn parse_double_brace(state: &mut RegexBuilder<'_>) {
    #[expect(clippy::indexing_slicing, reason = "predicate ensured bound")]
    let brace = state.bytes[state.position];
    state.push_literal_byte(brace);
    state.advance(2);
}

pub fn parse_literal(state: &mut RegexBuilder<'_>) {
    #[expect(clippy::indexing_slicing, reason = "caller ensured bound")]
    let ch = state.bytes[state.position];
    state.push_literal_byte(ch);
    state.advance(1);
}

#[must_use]
pub fn parse_placeholder_name(state: &RegexBuilder<'_>, start: usize) -> (usize, String) {
    let mut i = start + 1;
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

#[must_use]
pub fn parse_type_hint(state: &RegexBuilder<'_>, start: usize) -> (usize, Option<String>) {
    let mut i = start;
    if !matches!(state.bytes.get(i), Some(b':')) {
        return (i, None);
    }
    i += 1;
    let ty_start = i;
    while let Some(&b) = state.bytes.get(i) {
        if b == b'}' {
            break;
        }
        i += 1;
    }
    #[expect(clippy::string_slice, reason = "ASCII region delimited by braces")]
    let ty = state.pattern[ty_start..i].to_string();
    if ty.is_empty() {
        return (i, Some(String::new()));
    }
    (i, Some(ty))
}

fn placeholder_error(
    message: &'static str,
    position: usize,
    placeholder: Option<String>,
) -> PatternError {
    PatternError::Placeholder(PlaceholderErrorInfo::new(message, position, placeholder))
}

/// Ensure placeholder names do not contain whitespace before type hints.
///
/// # Errors
/// Returns [`PatternError::Placeholder`] when the placeholder syntax is invalid.
pub fn validate_placeholder_whitespace(
    state: &RegexBuilder<'_>,
    name_end: usize,
    start: usize,
    name: &str,
) -> Result<(), PatternError> {
    if let Some(b) = state.bytes.get(name_end) {
        if (*b as char).is_ascii_whitespace() {
            let mut ws = name_end;
            while let Some(bw) = state.bytes.get(ws) {
                if !(*bw as char).is_ascii_whitespace() {
                    break;
                }
                ws += 1;
            }
            if matches!(state.bytes.get(ws), Some(b':'))
                || matches!(state.bytes.get(ws), Some(b'}'))
            {
                return Err(placeholder_error(
                    "invalid placeholder in step pattern",
                    start,
                    Some(name.to_string()),
                ));
            }
        }
    }
    Ok(())
}

fn is_invalid_type_hint(ty: &str) -> bool {
    ty.is_empty()
        || ty.chars().any(|c| c.is_ascii_whitespace())
        || ty.contains('{')
        || ty.contains('}')
}

/// Validate the syntax of an optional placeholder type hint.
///
/// # Errors
/// Returns [`PatternError::Placeholder`] when the hint is empty or malformed.
pub fn validate_type_hint(
    ty_raw: Option<String>,
    start: usize,
    name: &str,
) -> Result<Option<String>, PatternError> {
    if let Some(ty) = ty_raw {
        if is_invalid_type_hint(&ty) {
            return Err(placeholder_error(
                "invalid placeholder in step pattern",
                start,
                Some(name.to_string()),
            ));
        }
        Ok(Some(ty))
    } else {
        Ok(None)
    }
}

#[must_use]
pub fn find_closing_brace(bytes: &[u8], start: usize) -> Option<usize> {
    let mut k = start;
    let mut nest = 0usize;
    while let Some(&b) = bytes.get(k) {
        match b {
            b'{' => {
                nest += 1;
                k += 1;
            }
            b'}' => {
                if nest == 0 {
                    return Some(k);
                }
                nest -= 1;
                k += 1;
            }
            _ => k += 1,
        }
    }
    None
}

/// Parse a placeholder at the builder's current position.
///
/// # Errors
/// Returns [`PatternError`] when the placeholder syntax or nesting is invalid.
pub fn parse_placeholder(state: &mut RegexBuilder<'_>) -> Result<(), PatternError> {
    let start = state.position;
    let (name_end, name) = parse_placeholder_name(state, start);
    validate_placeholder_whitespace(state, name_end, start, &name)?;
    let (mut after, ty_raw) = parse_type_hint(state, name_end);
    let ty_opt = validate_type_hint(ty_raw, start, &name)?;
    if ty_opt.is_none() {
        after = find_closing_brace(state.bytes, name_end).ok_or_else(|| {
            placeholder_error(
                "missing closing '}' for placeholder",
                start,
                Some(name.clone()),
            )
        })?;
    }
    if !matches!(state.bytes.get(after), Some(b'}')) {
        return Err(placeholder_error(
            "missing closing '}' for placeholder",
            start,
            Some(name),
        ));
    }
    state.push_capture_for_type(ty_opt.as_deref());
    after += 1;
    state.position = after;
    Ok(())
}

#[inline]
pub fn try_parse_common_sequences(st: &mut RegexBuilder<'_>) -> bool {
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
            st.push_literal_byte(b'\\');
            st.advance(1);
        }
        true
    } else {
        false
    }
}

#[inline]
pub fn parse_stray_character(st: &mut RegexBuilder<'_>) {
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

/// Parse context-sensitive sequences when building the regex.
///
/// # Errors
/// Returns [`PatternError`] for unmatched braces or malformed placeholders.
pub fn parse_context_specific(st: &mut RegexBuilder<'_>) -> Result<(), PatternError> {
    if st.stray_depth > 0 {
        parse_stray_character(st);
        return Ok(());
    }
    if is_placeholder_start(st.bytes, st.position) {
        return parse_placeholder(st);
    }
    match st.bytes.get(st.position) {
        Some(b'}') => Err(placeholder_error(
            "unmatched closing brace '}' in step pattern",
            st.position,
            None,
        )),
        Some(b'{') => {
            st.push_literal_byte(b'{');
            st.stray_depth = st.stray_depth.saturating_add(1);
            st.advance(1);
            Ok(())
        }
        _ => {
            parse_literal(st);
            Ok(())
        }
    }
}

/// Convert a step pattern into an anchored regular expression source.
///
/// # Errors
/// Returns [`PatternError`] when placeholder parsing fails or braces remain unbalanced.
pub fn build_regex_from_pattern(pat: &str) -> Result<String, PatternError> {
    let mut st = RegexBuilder::new(pat);
    while st.has_more() {
        if !try_parse_common_sequences(&mut st) {
            parse_context_specific(&mut st)?;
        }
    }
    if st.stray_depth != 0 {
        return Err(placeholder_error(
            "unbalanced braces in step pattern",
            st.position,
            None,
        ));
    }
    st.output.push('$');
    Ok(st.output)
}
