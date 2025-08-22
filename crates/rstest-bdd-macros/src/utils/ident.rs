//! Identifier utilities.

/// Sanitize a string so it may be used as a Rust identifier.
///
/// Only ASCII alphanumeric characters are retained; all other characters
/// (including Unicode) are replaced with underscores. The result is
/// lowercased. Identifiers starting with a digit gain a leading underscore,
/// and keywords are likewise prefixed to avoid collisions. See
/// [Rust Reference: Keywords](https://doc.rust-lang.org/reference/keywords.html)
/// for the full list of reserved words.
///
/// # Examples
///
/// ```rust,ignore
/// use crate::utils::ident::sanitize_ident;
/// assert_eq!(sanitize_ident("Crème—brûlée"), "cr_me_br_l_e");
/// assert_eq!(sanitize_ident("type"), "_type");
/// ```
pub(crate) fn sanitize_ident(input: &str) -> String {
    let mut ident = String::new();
    for c in input.chars() {
        if c.is_ascii_alphanumeric() {
            ident.push(c.to_ascii_lowercase());
        } else {
            ident.push('_');
        }
    }

    // Collapse repeated underscores to keep names tidy.
    let mut collapsed = String::with_capacity(ident.len());
    let mut prev_us = false;
    for ch in ident.chars() {
        if ch == '_' {
            if !prev_us {
                collapsed.push('_');
                prev_us = true;
            }
        } else {
            collapsed.push(ch);
            prev_us = false;
        }
    }

    // Trim trailing underscores that don't add meaning.
    let mut ident = collapsed.trim_end_matches('_').to_string();

    if ident.is_empty() || matches!(ident.chars().next(), Some(c) if c.is_ascii_digit()) {
        ident.insert(0, '_');
    }
    if RUST_KEYWORDS.contains(&ident.as_str()) {
        ident.insert(0, '_');
    }
    ident
}

/// Rust keywords that are invalid as identifiers.
const RUST_KEYWORDS: &[&str] = &[
    "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn", "for",
    "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return",
    "self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use", "where",
    "while", "async", "await", "dyn", "union", "abstract", "become", "box", "do", "final", "macro",
    "override", "priv", "try", "typeof", "unsized", "virtual", "yield",
];

#[cfg(test)]
mod tests {
    use super::sanitize_ident;

    #[test]
    fn sanitizes_invalid_identifiers() {
        assert_eq!(sanitize_ident("Hello world!"), "hello_world");
    }

    #[test]
    fn sanitizes_leading_digit() {
        assert_eq!(sanitize_ident("123abc"), "_123abc");
    }

    #[test]
    fn sanitizes_empty_input() {
        assert_eq!(sanitize_ident(""), "_");
    }

    #[test]
    fn sanitizes_unicode() {
        assert_eq!(sanitize_ident("Crème—brûlée"), "cr_me_br_l_e");
    }

    #[test]
    fn sanitizes_rust_keywords() {
        assert_eq!(sanitize_ident("fn"), "_fn");
        assert_eq!(sanitize_ident("type"), "_type");
        assert_eq!(sanitize_ident("Self"), "_self");
    }

    #[test]
    fn collapses_repeated_underscores() {
        assert_eq!(sanitize_ident("a--b__c"), "a_b_c");
    }
}
