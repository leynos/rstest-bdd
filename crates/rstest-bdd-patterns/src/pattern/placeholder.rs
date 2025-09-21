//! Placeholder parsing utilities used by the lexer.

use crate::errors::{PatternError, placeholder_error};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlaceholderSpec {
    pub name: String,
    pub hint: Option<String>,
    pub start: usize,
    pub end: usize,
}

const BACKSLASH: u8 = 92;
const OPEN_BRACE: u8 = 123;
const CLOSE_BRACE: u8 = 125;
const COLON: u8 = 58;

fn find_closing_brace(bytes: &[u8], start: usize) -> Option<usize> {
    let mut index = start;
    let mut depth = 0usize;
    while let Some(&b) = bytes.get(index) {
        match b {
            OPEN_BRACE => {
                depth = depth.saturating_add(1);
                index += 1;
            }
            CLOSE_BRACE => {
                if depth == 0 {
                    return Some(index);
                }
                depth -= 1;
                index += 1;
            }
            _ => index += 1,
        }
    }
    None
}

pub(crate) fn parse_placeholder(
    bytes: &[u8],
    start: usize,
) -> Result<(usize, PlaceholderSpec), PatternError> {
    let mut index = start + 1;
    let name = parse_name(bytes, &mut index);
    skip_forbidden_whitespace(bytes, &mut index, start, &name)?;

    let hint = parse_optional_hint(bytes, &mut index, start, &name)?;

    let closing_index = if hint.is_some() {
        index
    } else {
        find_closing_brace(bytes, index).ok_or_else(|| {
            placeholder_error(
                "missing closing '}' for placeholder",
                start,
                Some(name.clone()),
            )
        })?
    };

    if bytes.get(closing_index).copied() != Some(CLOSE_BRACE) {
        return Err(placeholder_error(
            "missing closing '}' for placeholder",
            start,
            Some(name.clone()),
        ));
    }

    let end = closing_index + 1;

    Ok((
        end,
        PlaceholderSpec {
            name,
            hint,
            start,
            end,
        },
    ))
}

fn parse_name(bytes: &[u8], index: &mut usize) -> String {
    let mut name = String::new();
    while let Some(&b) = bytes.get(*index) {
        let ch = b as char;
        if ch.is_ascii_alphanumeric() || b == b'_' {
            name.push(ch);
            *index += 1;
        } else {
            break;
        }
    }
    name
}

fn skip_forbidden_whitespace(
    bytes: &[u8],
    index: &mut usize,
    start: usize,
    name: &str,
) -> Result<(), PatternError> {
    let Some(&next) = bytes.get(*index) else {
        return Ok(());
    };

    if !(next as char).is_ascii_whitespace() {
        return Ok(());
    }

    let mut end = *index;
    while let Some(&b) = bytes.get(end) {
        if !(b as char).is_ascii_whitespace() {
            break;
        }
        end += 1;
    }

    let Some(&next_byte) = bytes.get(end) else {
        *index = end;
        return Ok(());
    };

    if next_byte == COLON || next_byte == CLOSE_BRACE {
        return Err(placeholder_error(
            "invalid placeholder in step pattern",
            start,
            Some(name.to_string()),
        ));
    }

    *index = end;
    Ok(())
}

fn parse_optional_hint(
    bytes: &[u8],
    index: &mut usize,
    start: usize,
    name: &str,
) -> Result<Option<String>, PatternError> {
    if bytes.get(*index).copied() != Some(b':') {
        return Ok(None);
    }

    *index += 1;
    let (end, raw_bytes) = extract_hint_bytes(bytes, *index, start, name)?;
    let raw = parse_hint_text(raw_bytes, start, name)?;

    if !is_valid_hint_format(raw) {
        return Err(placeholder_error(
            "invalid placeholder in step pattern",
            start,
            Some(name.to_string()),
        ));
    }

    *index = end;
    Ok(Some(raw.to_string()))
}

/// Determine whether a backslash escapes a brace inside a placeholder hint.
///
/// The helper inspects the byte following the backslash and returns ``true``
/// when it would produce an escaped `{` or `}` character.
fn is_invalid_escape_sequence(bytes: &[u8], backslash_pos: usize) -> bool {
    matches!(
        bytes.get(backslash_pos + 1),
        Some(&OPEN_BRACE | &CLOSE_BRACE)
    )
}

/// Extract the raw bytes that make up a placeholder hint.
///
/// The slice begins immediately after the colon and stops before the closing
/// brace. Nested `{` braces or escaped braces (a backslash immediately followed
/// by `{` or `}`) are rejected because hints map to Rust type identifiers. A
/// [`PatternError`] is returned if the hint spans invalid syntax or the closing
/// brace is missing.
///
/// # Examples
/// ```ignore
/// let bytes = b"{value:u32}";
/// let (end, hint) = extract_hint_bytes(bytes, 7, 0, "value").unwrap();
/// assert_eq!(end, 10);
/// assert_eq!(hint, b"u32");
/// ```
fn extract_hint_bytes<'a>(
    bytes: &'a [u8],
    hint_start: usize,
    start: usize,
    name: &str,
) -> Result<(usize, &'a [u8]), PatternError> {
    let mut end = hint_start;

    loop {
        let Some(&byte) = bytes.get(end) else {
            return Err(placeholder_error(
                "missing closing '}' for placeholder",
                start,
                Some(name.to_string()),
            ));
        };

        if byte == CLOSE_BRACE {
            break;
        }

        if byte == OPEN_BRACE || (byte == BACKSLASH && is_invalid_escape_sequence(bytes, end)) {
            return Err(placeholder_error(
                "invalid placeholder in step pattern",
                start,
                Some(name.to_string()),
            ));
        }

        end += 1;
    }

    let raw_bytes = bytes.get(hint_start..end).ok_or_else(|| {
        placeholder_error(
            "invalid placeholder in step pattern",
            start,
            Some(name.to_string()),
        )
    })?;

    Ok((end, raw_bytes))
}

/// Convert the raw UTF-8 bytes of a placeholder hint into text.
///
/// A [`PatternError`] is returned when the bytes are not valid UTF-8 so the
/// caller can report the placeholder span precisely.
///
/// # Examples
/// ```ignore
/// let raw = b"u32";
/// let hint = parse_hint_text(raw, 0, "value").unwrap();
/// assert_eq!(hint, "u32");
/// ```
/// ```ignore
/// assert!(parse_hint_text(&[0xFF], 0, "value").is_err());
/// ```
fn parse_hint_text<'a>(
    raw_bytes: &'a [u8],
    start: usize,
    name: &str,
) -> Result<&'a str, PatternError> {
    std::str::from_utf8(raw_bytes).map_err(|_| {
        placeholder_error(
            "invalid placeholder in step pattern",
            start,
            Some(name.to_string()),
        )
    })
}

/// Check whether a parsed hint string satisfies formatting rules.
///
/// Valid hints are non-empty, contain no ASCII whitespace, and avoid braces so
/// they can be embedded directly into generated Rust code. Returns ``true`` for
/// hints that meet these constraints.
///
/// # Examples
/// ```ignore
/// assert!(is_valid_hint_format("u32"));
/// assert!(!is_valid_hint_format("bad hint"));
/// assert!(!is_valid_hint_format("{bad}"));
/// ```
fn is_valid_hint_format(hint: &str) -> bool {
    !hint.is_empty()
        && !hint.chars().any(|c| c.is_ascii_whitespace())
        && !hint.contains('{')
        && !hint.contains('}')
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "tests exercise placeholder parser fallibility"
)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_placeholder() {
        let pattern = "{value}";
        let (next, spec) = parse_placeholder(pattern.as_bytes(), 0).unwrap();
        assert_eq!(next, pattern.len());
        assert_eq!(spec.name, "value");
        assert_eq!(spec.hint, None);
    }

    #[test]
    fn parses_placeholder_with_type_hint() {
        let pattern = "{value:u32}";
        let (next, spec) = parse_placeholder(pattern.as_bytes(), 0).unwrap();
        assert_eq!(next, pattern.len());
        assert_eq!(spec.name, "value");
        assert_eq!(spec.hint.as_deref(), Some("u32"));
    }

    #[test]
    fn parses_placeholder_with_nested_braces() {
        let pattern = "{outer {inner}}";
        let (next, spec) = parse_placeholder(pattern.as_bytes(), 0).unwrap();
        assert_eq!(next, pattern.len());
        assert_eq!(spec.name, "outer");
        assert_eq!(spec.hint, None);
    }

    #[test]
    fn errors_on_missing_closing_brace() {
        let pattern = "{value";
        let err = parse_placeholder(pattern.as_bytes(), 0).unwrap_err();
        assert!(err.to_string().contains("missing closing"));
    }

    #[test]
    fn errors_on_whitespace_before_hint() {
        let pattern = "{value :u32}";
        let err = parse_placeholder(pattern.as_bytes(), 0).unwrap_err();
        assert!(
            err.to_string()
                .contains("invalid placeholder in step pattern")
        );
    }

    #[test]
    fn validates_hint_format_rules() {
        assert!(is_valid_hint_format("u32"));
        assert!(is_valid_hint_format("serde::JsonValue"));
        assert!(!is_valid_hint_format(""));
        assert!(!is_valid_hint_format("bad hint"));
        assert!(!is_valid_hint_format("{bad"));
        assert!(!is_valid_hint_format("bad}"));
    }

    #[test]
    fn errors_on_hint_with_nested_brace() {
        let pattern = "{value:Vec<{u32}>}";
        let err = parse_placeholder(pattern.as_bytes(), 0).unwrap_err();
        assert!(err.to_string().contains("invalid placeholder"));
    }

    #[test]
    fn errors_on_hint_with_escaped_brace() {
        let pattern = format!("{{value:{esc}{{hint{esc}}}}}", esc = char::from(BACKSLASH));
        let err = parse_placeholder(pattern.as_bytes(), 0).unwrap_err();
        assert!(err.to_string().contains("invalid placeholder"));
    }
}
