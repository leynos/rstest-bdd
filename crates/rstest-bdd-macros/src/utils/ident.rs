//! Identifier utilities.

/// Sanitize a string so it may be used as a Rust identifier.
///
/// Only ASCII alphanumeric characters are retained; all other characters
/// (including Unicode) are replaced with underscores. Runs of underscores are
/// collapsed to a single `_`, and trailing underscores are trimmed. The result
/// is lowercased. Identifiers starting with a digit gain a leading underscore,
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
/// assert_eq!(sanitize_ident("hello!"), "hello");
/// ```
pub(crate) fn sanitize_ident(input: &str) -> String {
    let ident = replace_non_ascii_with_underscores(input);
    let ident = trim_trailing_underscores(&ident);
    add_prefix_if_needed(ident)
}

fn replace_non_ascii_with_underscores(input: &str) -> String {
    // Single pass: map to ASCII, collapse repeated underscores on the fly.
    let mut ident = String::with_capacity(input.len());
    let mut prev_us = false;
    for c in input.chars() {
        if c.is_ascii_alphanumeric() {
            ident.push(c.to_ascii_lowercase());
            prev_us = false;
        } else {
            prev_us = should_add_underscore(&mut ident, prev_us);
        }
    }
    ident
}

fn should_add_underscore(ident: &mut String, prev_us: bool) -> bool {
    // Append a single underscore unless the previous character already was one.
    if !prev_us {
        ident.push('_');
    }
    true
}

fn trim_trailing_underscores(input: &str) -> String {
    // Trim trailing underscores that don't add meaning.
    input.trim_end_matches('_').to_string()
}

fn add_prefix_if_needed(mut ident: String) -> String {
    if needs_underscore_prefix(&ident) {
        ident.insert(0, '_');
    }
    ident
}

fn needs_underscore_prefix(ident: &str) -> bool {
    ident.is_empty()
        || ident.chars().next().is_some_and(|c| c.is_ascii_digit())
        || RUST_KEYWORDS.contains(&ident)
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

    #[test]
    fn sanitizes_all_non_alnum() {
        assert_eq!(sanitize_ident("!!!"), "_");
        assert_eq!(sanitize_ident("🙈🙉🙊"), "_");
    }

    #[test]
    fn trims_trailing_underscores() {
        assert_eq!(sanitize_ident("hello!"), "hello");
        assert_eq!(sanitize_ident("end__"), "end");
    }
}
