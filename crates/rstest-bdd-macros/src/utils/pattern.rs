//! Pattern utilities for compile-time analysis.
//!
//! Provides helper to extract placeholder names from step patterns so the macro
//! can distinguish fixtures from step arguments. The parser is intentionally
//! minimal and recognises the same escape rules as the runtime pattern parser.

use std::collections::HashSet;

use syn::Result;

/// Extract placeholder identifiers from a pattern string.
///
/// The function scans the pattern for segments of the form `{name}` or
/// `{name:type}` and returns the set of placeholder names. Escaped braces and
/// doubled braces are treated as literals.
///
/// # Errors
/// Returns a [`syn::Error`] when the pattern contains unbalanced or stray
/// braces.
pub(crate) fn placeholder_names(pattern: &str) -> Result<HashSet<String>> {
    let bytes = pattern.as_bytes();
    let mut names = HashSet::new();
    let mut i = 0;

    while let Some(&b) = bytes.get(i) {
        match b {
            b'\\' => i = i.saturating_add(2),
            b'{' => {
                if bytes.get(i + 1) == Some(&b'{') {
                    i += 2;
                    continue;
                }

                let (name, next) = parse_placeholder(bytes, i)?;
                names.insert(name);
                i = next;
            }
            b'}' => {
                if bytes.get(i + 1) == Some(&b'}') {
                    i += 2;
                    continue;
                }
                return Err(syn::Error::new(
                    proc_macro2::Span::call_site(),
                    "unmatched '}' in step pattern",
                ));
            }
            _ => i += 1,
        }
    }

    Ok(names)
}

/// Parse a placeholder starting at `start`, returning the name and the index of
/// the next character after the closing brace.
fn parse_placeholder(bytes: &[u8], start: usize) -> Result<(String, usize)> {
    let mut j = start + 1;

    let first = *bytes.get(j).ok_or_else(|| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            "unmatched '{' in step pattern",
        )
    })?;
    if !first.is_ascii_alphabetic() && first != b'_' {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "unmatched '{' in step pattern",
        ));
    }
    j += 1;
    while let Some(&b) = bytes.get(j) {
        if b.is_ascii_alphanumeric() || b == b'_' {
            j += 1;
        } else {
            break;
        }
    }

    let slice = bytes.get(start + 1..j).ok_or_else(|| {
        syn::Error::new(proc_macro2::Span::call_site(), "invalid placeholder range")
    })?;
    let name = std::str::from_utf8(slice).map_err(|_| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            "placeholder name must be valid UTF-8",
        )
    })?;

    if bytes.get(j) == Some(&b':') {
        j += 1;
        while let Some(&b) = bytes.get(j) {
            if b == b'}' {
                break;
            }
            if b == b'{' {
                return Err(syn::Error::new(
                    proc_macro2::Span::call_site(),
                    "unmatched '{' in type hint",
                ));
            }
            j += 1;
        }
    }

    if bytes.get(j) != Some(&b'}') {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "unbalanced braces in step pattern",
        ));
    }

    Ok((name.to_string(), j + 1))
}
