//! Identifier utilities.

/// Sanitize a string so it may be used as a Rust identifier.
///
/// Only ASCII alphanumeric characters are retained; all other characters
/// (including Unicode) are replaced with underscores. The result is
/// lowercased. Identifiers starting with a digit gain a leading underscore.
///
/// # Examples
///
/// ```rust,ignore
/// use crate::utils::ident::sanitize_ident;
/// assert_eq!(sanitize_ident("Crème—brûlée"), "cr_me_br_l_e");
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
    if ident.is_empty() || matches!(ident.chars().next(), Some(c) if c.is_ascii_digit()) {
        ident.insert(0, '_');
    }
    ident
}

#[cfg(test)]
mod tests {
    use super::sanitize_ident;

    #[test]
    fn sanitizes_invalid_identifiers() {
        assert_eq!(sanitize_ident("Hello world!"), "hello_world_");
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
}
