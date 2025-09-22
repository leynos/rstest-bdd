//! Placeholder type-hint helpers used during regex compilation.

/// Translate a placeholder type hint into a regular-expression fragment.
///
/// # Examples
/// ```ignore
/// use rstest_bdd_patterns::get_type_pattern;
/// assert_eq!(get_type_pattern(Some("u32")), "\d+");
/// assert_eq!(get_type_pattern(Some("f64")), "(?i:(?:[+-]?(?:\d+\.\d*|\.\d+|\d+)(?:[eE][+-]?\d+)?|nan|inf|infinity))");
/// assert_eq!(get_type_pattern(None), ".+?");
/// ```
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_integer_pattern_for_unsigned_types() {
        assert_eq!(get_type_pattern(Some("u64")), r"\d+");
    }

    #[test]
    fn returns_signed_integer_pattern() {
        assert_eq!(get_type_pattern(Some("i32")), r"[+-]?\d+");
    }

    #[test]
    fn returns_float_pattern() {
        assert_eq!(
            get_type_pattern(Some("f32")),
            r"(?i:(?:[+-]?(?:\d+\.\d*|\.\d+|\d+)(?:[eE][+-]?\d+)?|nan|inf|infinity))"
        );
    }

    #[test]
    fn defaults_to_lazy_match_for_unknown_types() {
        assert_eq!(get_type_pattern(Some("String")), r".+?");
    }
}
