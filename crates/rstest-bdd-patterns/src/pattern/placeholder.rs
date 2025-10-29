//! Placeholder parsing utilities used by the lexer.

use crate::errors::{placeholder_error, PatternError};

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

/// Scan `bytes` starting at `start` (immediately after `{name` or `{name:hint`) and
/// return the index of the matching `}` for the placeholder, honouring nested
/// braces.
fn find_closing_brace(bytes: &[u8], start: usize) -> Option<usize> {
    debug_assert!(
        start <= bytes.len(),
        "start must not exceed the available input",
    );
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

struct PlaceholderContext<'a> {
    bytes: &'a [u8],
    start: usize,
    name: &'a str,
}

pub(crate) fn parse_placeholder(
    bytes: &[u8],
    start: usize,
) -> Result<(usize, PlaceholderSpec), PatternError> {
    let mut index = start + 1;
    let name = parse_name(bytes, &mut index);
    let ctx = PlaceholderContext {
        bytes,
        start,
        name: &name,
    };

    skip_forbidden_whitespace(&ctx, &mut index)?;

    let hint = parse_optional_hint(&ctx, &mut index)?;

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
    ctx: &PlaceholderContext,
    index: &mut usize,
) -> Result<(), PatternError> {
    let Some(&next_byte) = ctx.bytes.get(*index) else {
        return Ok(());
    };

    if !(next_byte as char).is_ascii_whitespace() {
        return Ok(());
    }

    let end = skip_all_whitespace(ctx.bytes, *index);

    if has_forbidden_byte_after_whitespace(ctx, end) {
        return Err(placeholder_error(
            "invalid placeholder in step pattern",
            ctx.start,
            Some(ctx.name.to_string()),
        ));
    }

    *index = end;
    Ok(())
}

fn skip_all_whitespace(bytes: &[u8], mut index: usize) -> usize {
    while let Some(&byte) = bytes.get(index) {
        if !(byte as char).is_ascii_whitespace() {
            break;
        }
        index += 1;
    }
    index
}

fn has_forbidden_byte_after_whitespace(ctx: &PlaceholderContext, index: usize) -> bool {
    matches!(ctx.bytes.get(index), Some(&COLON | &CLOSE_BRACE))
}

fn parse_optional_hint(
    ctx: &PlaceholderContext,
    index: &mut usize,
) -> Result<Option<String>, PatternError> {
    if ctx.bytes.get(*index).copied() != Some(COLON) {
        return Ok(None);
    }

    *index += 1;
    let (end, raw_bytes) = extract_hint_bytes(ctx, *index)?;
    let raw = parse_hint_text(raw_bytes, ctx.start, ctx.name)?;

    if !is_valid_hint_format(raw) {
        return Err(placeholder_error(
            "invalid placeholder in step pattern",
            ctx.start,
            Some(ctx.name.to_string()),
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
/// let ctx = PlaceholderContext { bytes, start: 0, name: "value" };
/// let (end, hint) = extract_hint_bytes(&ctx, 7).unwrap();
/// assert_eq!(end, 10);
/// assert_eq!(hint, b"u32");
/// ```
fn extract_hint_bytes<'a>(
    ctx: &PlaceholderContext<'a>,
    hint_start: usize,
) -> Result<(usize, &'a [u8]), PatternError> {
    let mut end = hint_start;

    loop {
        let Some(&byte) = ctx.bytes.get(end) else {
            return Err(placeholder_error(
                "missing closing '}' for placeholder",
                ctx.start,
                Some(ctx.name.to_string()),
            ));
        };

        if byte == CLOSE_BRACE {
            break;
        }

        if byte == OPEN_BRACE {
            return Err(placeholder_error(
                "invalid placeholder in step pattern",
                ctx.start,
                Some(ctx.name.to_string()),
            ));
        }

        if byte == BACKSLASH && is_invalid_escape_sequence(ctx.bytes, end) {
            return Err(placeholder_error(
                "invalid placeholder in step pattern",
                ctx.start,
                Some(ctx.name.to_string()),
            ));
        }

        end += 1;
    }

    let raw_bytes = ctx.bytes.get(hint_start..end).ok_or_else(|| {
        placeholder_error(
            "invalid placeholder in step pattern",
            ctx.start,
            Some(ctx.name.to_string()),
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
mod tests {
    use super::*;
    use crate::pattern::test_support::{parse_err, parse_ok};

    #[test]
    fn parses_basic_placeholder() {
        let pattern = "{value}";
        let (next, spec) = parse_ok(pattern);
        assert_eq!(next, pattern.len());
        assert_eq!(spec.name, "value");
        assert_eq!(spec.hint, None);
    }

    #[test]
    fn parses_placeholder_with_type_hint() {
        let pattern = "{value:u32}";
        let (next, spec) = parse_ok(pattern);
        assert_eq!(next, pattern.len());
        assert_eq!(spec.name, "value");
        assert_eq!(spec.hint.as_deref(), Some("u32"));
    }

    #[test]
    fn parses_placeholder_with_nested_braces() {
        let pattern = "{outer {inner}}";
        let (next, spec) = parse_ok(pattern);
        assert_eq!(next, pattern.len());
        assert_eq!(spec.name, "outer");
        assert_eq!(spec.hint, None);
    }

    #[test]
    fn errors_on_missing_closing_brace() {
        let pattern = "{value";
        let err = parse_err(pattern);
        assert!(err.to_string().contains("missing closing"));
    }

    #[test]
    fn errors_on_whitespace_before_hint() {
        let pattern = "{value :u32}";
        let err = parse_err(pattern);
        assert!(err
            .to_string()
            .contains("invalid placeholder in step pattern"));
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
        let err = parse_err(pattern);
        assert!(err.to_string().contains("invalid placeholder"));
    }

    #[test]
    fn errors_on_hint_with_escaped_brace() {
        let pattern = format!("{{value:{esc}{{hint{esc}}}}}", esc = char::from(BACKSLASH));
        let err = parse_err(&pattern);
        assert!(err.to_string().contains("invalid placeholder"));
    }
}
