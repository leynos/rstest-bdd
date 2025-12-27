//! Pattern utilities for compile-time analysis.
//!
//! Provides helper to extract placeholder names from step patterns so the macro
//! can distinguish fixtures from step arguments. The parser is intentionally
//! minimal and recognises the same escape rules as the runtime pattern parser.
//!
//! Also provides name normalisation for underscore-prefixed parameters, enabling
//! `_param` to match placeholder `param` for idiomatic unused parameter marking.

use std::collections::HashSet;

use syn::{Ident, LitStr, Result};

/// Information about a single placeholder extracted from a pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlaceholderInfo {
    /// The placeholder name (e.g., `args` from `{args:string}`).
    pub name: String,
    /// The optional type hint (e.g., `string` from `{args:string}`).
    pub hint: Option<String>,
}

/// Ordered and deduplicated placeholder information extracted from a pattern.
pub(crate) struct PlaceholderSummary {
    /// Placeholder info in textual order (duplicates preserved).
    pub ordered: Vec<PlaceholderInfo>,
    /// Unique placeholder names used for parameter classification.
    pub unique: HashSet<String>,
}

/// Extract placeholder identifiers from a pattern string.
///
/// The function scans the pattern for segments of the form `{name}` or
/// `{name:type}` and returns the set of placeholder names. Escaped braces and
/// doubled braces are treated as literals.
///
/// # Errors
/// Returns a [`syn::Error`] when the pattern contains unbalanced or stray
/// braces.
pub(crate) fn placeholder_names(pattern: &str) -> Result<PlaceholderSummary> {
    let bytes = pattern.as_bytes();
    let mut names = HashSet::new();
    let mut ordered = Vec::new();
    let mut i = 0;

    while let Some(&b) = bytes.get(i) {
        match b {
            b'\\' => i = i.saturating_add(2),
            b'{' => {
                if bytes.get(i + 1) == Some(&b'{') {
                    i += 2;
                    continue;
                }

                let (info, next) = parse_placeholder(bytes, i)?;
                let _ = names.insert(info.name.clone());
                ordered.push(info);
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

    Ok(PlaceholderSummary {
        ordered,
        unique: names,
    })
}

/// Parse a placeholder starting at `start`, returning the info and the index of
/// the next character after the closing brace.
///
/// # Examples
/// ```ignore
/// let pattern = b"{world}";
/// let (info, end) = parse_placeholder(pattern, 0).unwrap();
/// assert_eq!(info.name, "world");
/// assert_eq!(info.hint, None);
/// assert_eq!(end, 7);
/// ```
fn parse_placeholder(bytes: &[u8], start: usize) -> Result<(PlaceholderInfo, usize)> {
    let mut j = start + 1;
    j = parse_placeholder_name(bytes, j)?;
    let name = extract_placeholder_name(bytes, start + 1, j)?;
    let (hint, j) = extract_type_hint_if_present(bytes, j)?;
    validate_closing_brace(bytes, j)?;
    Ok((
        PlaceholderInfo {
            name: name.to_string(),
            hint,
        },
        j + 1,
    ))
}

/// Parse the identifier portion of a placeholder, returning the index after the
/// name.
///
/// # Examples
/// ```ignore
/// let bytes = b"{foo}";
/// let end = parse_placeholder_name(bytes, 1).unwrap();
/// assert_eq!(end, 4);
/// ```
fn parse_placeholder_name(bytes: &[u8], mut j: usize) -> Result<usize> {
    let first = *bytes.get(j).ok_or_else(|| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            "unmatched '{' in step pattern",
        )
    })?;
    if !is_valid_name_start(first) {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "invalid placeholder name start (expected ASCII letter or '_')",
        ));
    }
    j += 1;
    while let Some(&b) = bytes.get(j) {
        if is_valid_name_char(b) {
            j += 1;
        } else {
            break;
        }
    }
    Ok(j)
}

/// Extract the placeholder name slice and ensure it is valid UTF-8.
///
/// # Examples
/// ```ignore
/// let bytes = b"{foo}";
/// let name = extract_placeholder_name(bytes, 1, 4).unwrap();
/// assert_eq!(name, "foo");
/// ```
fn extract_placeholder_name(bytes: &[u8], start: usize, end: usize) -> Result<&str> {
    let slice = bytes.get(start..end).ok_or_else(|| {
        syn::Error::new(proc_macro2::Span::call_site(), "invalid placeholder range")
    })?;
    let name = std::str::from_utf8(slice).map_err(|_| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            "placeholder name must be valid UTF-8",
        )
    })?;
    Ok(name)
}

/// Extract an optional `:type` hint, returning the hint value and the index of
/// the closing brace or the character that should be the closing brace.
///
/// # Examples
/// ```ignore
/// let bytes = b"{foo:bar}";
/// let (hint, end) = extract_type_hint_if_present(bytes, 4).unwrap();
/// assert_eq!(hint, Some("bar".to_string()));
/// assert_eq!(end, 8);
/// ```
fn extract_type_hint_if_present(bytes: &[u8], mut j: usize) -> Result<(Option<String>, usize)> {
    if bytes.get(j) != Some(&b':') {
        return Ok((None, j));
    }

    let hint_start = j + 1;
    j = hint_start;

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

    let hint_slice = bytes.get(hint_start..j).ok_or_else(|| {
        syn::Error::new(proc_macro2::Span::call_site(), "invalid type hint range")
    })?;
    let hint = std::str::from_utf8(hint_slice)
        .map_err(|_| {
            syn::Error::new(
                proc_macro2::Span::call_site(),
                "type hint must be valid UTF-8",
            )
        })?
        .to_string();

    // Return None for empty hints (just a trailing colon with no content)
    let hint = if hint.is_empty() { None } else { Some(hint) };
    Ok((hint, j))
}

/// Ensure the placeholder ends with a closing brace.
///
/// # Examples
/// ```ignore
/// let bytes = b"{foo}";
/// validate_closing_brace(bytes, 4).unwrap();
/// ```
fn validate_closing_brace(bytes: &[u8], j: usize) -> Result<()> {
    if bytes.get(j) != Some(&b'}') {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "unbalanced braces in step pattern",
        ));
    }
    Ok(())
}

/// Determine whether `b` may start an identifier.
///
/// # Examples
/// ```ignore
/// assert!(is_valid_name_start(b'f'));
/// assert!(!is_valid_name_start(b'1'));
/// ```
fn is_valid_name_start(b: u8) -> bool {
    // ASCII-only: start must be a letter or underscore.
    b.is_ascii_alphabetic() || b == b'_'
}

/// Determine whether `b` may appear after the first character of an identifier.
///
/// # Examples
/// ```ignore
/// assert!(is_valid_name_char(b'1'));
/// assert!(!is_valid_name_char(b'-'));
/// ```
fn is_valid_name_char(b: u8) -> bool {
    // Subsequent identifier characters may also include digits.
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Infer a step pattern from a function identifier by replacing underscores with spaces.
///
/// # Examples
/// ```rust,ignore
/// use syn::parse_quote;
/// let ident: syn::Ident = parse_quote!(user_logs_in);
/// let pattern = infer_pattern(&ident);
/// assert_eq!(pattern.value(), "user logs in");
/// ```
pub(crate) fn infer_pattern(ident: &Ident) -> LitStr {
    // Strip raw identifier prefix if present to avoid `r#` in user-visible patterns.
    let mut name = ident.to_string();
    if let Some(stripped) = name.strip_prefix("r#") {
        name = stripped.to_owned();
    }
    let inferred = name.replace('_', " ");
    LitStr::new(&inferred, ident.span())
}

/// Strip a single leading underscore from a parameter name for matching.
///
/// This enables idiomatic Rust unused parameter marking: `_param` matches
/// placeholder `param`. Only one underscore is stripped (`__param` becomes
/// `_param`) to preserve Rust's double-underscore convention.
///
/// # Examples
/// ```rust,ignore
/// assert_eq!(normalize_param_name("_param"), "param");
/// assert_eq!(normalize_param_name("param"), "param");
/// assert_eq!(normalize_param_name("__param"), "_param");
/// ```
pub(crate) fn normalize_param_name(name: &str) -> &str {
    name.strip_prefix('_').unwrap_or(name)
}

/// Check if an identifier matches a header after normalisation.
///
/// Compares the identifier to the header, applying the same underscore-stripping
/// logic as [`normalize_param_name`]. If the ident starts with `_`, compares the
/// suffix to the header; otherwise compares directly.
///
/// # Examples
/// ```rust,ignore
/// use syn::parse_quote;
/// let ident: syn::Ident = parse_quote!(_param);
/// assert!(ident_matches_normalized(&ident, "param"));
/// ```
pub(crate) fn ident_matches_normalized(ident: &Ident, header: &str) -> bool {
    // Check for leading underscore and compare normalized form to header.
    // This still requires to_string(), but consolidates the logic for matching.
    let ident_str = ident.to_string();
    normalize_param_name(&ident_str) == header
}

#[cfg(test)]
mod tests;
