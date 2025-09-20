//! Placeholder parsing utilities used by the lexer.

use crate::errors::{PatternError, placeholder_error};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlaceholderSpec {
    pub name: String,
    pub hint: Option<String>,
    pub start: usize,
    pub end: usize,
}

fn find_closing_brace(bytes: &[u8], start: usize) -> Option<usize> {
    let mut index = start;
    let mut depth = 0usize;
    while let Some(&b) = bytes.get(index) {
        match b {
            b'{' => {
                depth = depth.saturating_add(1);
                index += 1;
            }
            b'}' => {
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
    let mut name = String::new();
    while let Some(&b) = bytes.get(index) {
        let ch = b as char;
        if ch.is_ascii_alphanumeric() || b == b'_' {
            name.push(ch);
            index += 1;
        } else {
            break;
        }
    }

    if let Some(&b) = bytes.get(index) {
        if (b as char).is_ascii_whitespace() {
            let mut ws_end = index;
            while let Some(&bw) = bytes.get(ws_end) {
                if !(bw as char).is_ascii_whitespace() {
                    break;
                }
                ws_end += 1;
            }
            if matches!(bytes.get(ws_end), Some(b':' | b'}')) {
                return Err(placeholder_error(
                    "invalid placeholder in step pattern",
                    start,
                    Some(name),
                ));
            }
            index = ws_end;
        }
    }

    let mut hint = None;
    if matches!(bytes.get(index), Some(b':')) {
        index += 1;
        let hint_start = index;
        while let Some(&b) = bytes.get(index) {
            if b == b'}' {
                break;
            }
            index += 1;
        }
        let raw_bytes = bytes.get(hint_start..index).ok_or_else(|| {
            placeholder_error(
                "invalid placeholder in step pattern",
                start,
                Some(name.clone()),
            )
        })?;
        let raw = std::str::from_utf8(raw_bytes).map_err(|_| {
            placeholder_error(
                "invalid placeholder in step pattern",
                start,
                Some(name.clone()),
            )
        })?;
        if raw.is_empty()
            || raw.chars().any(|c| c.is_ascii_whitespace())
            || raw.contains('{')
            || raw.contains('}')
        {
            return Err(placeholder_error(
                "invalid placeholder in step pattern",
                start,
                Some(name),
            ));
        }
        hint = Some(raw.to_string());
    } else {
        index = find_closing_brace(bytes, index).ok_or_else(|| {
            placeholder_error(
                "missing closing '}' for placeholder",
                start,
                Some(name.clone()),
            )
        })?;
    }

    if !matches!(bytes.get(index), Some(b'}')) {
        return Err(placeholder_error(
            "missing closing '}' for placeholder",
            start,
            Some(name),
        ));
    }
    index += 1;

    Ok((
        index,
        PlaceholderSpec {
            name,
            hint,
            start,
            end: index,
        },
    ))
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
}
