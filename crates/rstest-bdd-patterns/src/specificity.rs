//! Pattern specificity calculation for disambiguation.
//!
//! When multiple step patterns match the same step text, this module provides
//! scoring to select the most specific match. More specific patterns have more
//! literal text and fewer placeholders.

use crate::PatternError;
use crate::pattern::lexer::{Token, lex_pattern};
use std::cmp::Ordering;

/// Specificity score for a step pattern.
///
/// Used to rank patterns when multiple match the same step text. Higher scores
/// indicate more specific patterns that should take precedence.
///
/// # Ordering
///
/// Patterns are compared by:
/// 1. More literal characters → more specific
/// 2. Fewer placeholders → more specific
/// 3. More typed placeholders → more specific (tiebreaker)
///
/// # Examples
///
/// ```
/// use rstest_bdd_patterns::SpecificityScore;
///
/// let specific = SpecificityScore::calculate("the output is foo")
///     .expect("valid specific pattern");
/// let generic = SpecificityScore::calculate("the output is {value}")
///     .expect("valid generic pattern");
/// assert!(specific > generic);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SpecificityScore {
    /// Total number of literal characters in the pattern.
    pub literal_chars: usize,
    /// Number of placeholder tokens in the pattern.
    pub placeholder_count: usize,
    /// Number of placeholders with type hints (e.g., `{n:u32}`).
    pub typed_placeholder_count: usize,
}

impl SpecificityScore {
    /// Calculate the specificity score for a pattern string.
    ///
    /// # Errors
    ///
    /// Returns [`PatternError`] if the pattern contains invalid syntax.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd_patterns::SpecificityScore;
    ///
    /// let score = SpecificityScore::calculate("I have {count:u32} apples")
    ///     .expect("valid pattern");
    /// assert_eq!(score.literal_chars, 14); // "I have " + " apples"
    /// assert_eq!(score.placeholder_count, 1);
    /// assert_eq!(score.typed_placeholder_count, 1);
    /// ```
    pub fn calculate(pattern: &str) -> Result<Self, PatternError> {
        let tokens = lex_pattern(pattern)?;

        let mut literal_chars = 0usize;
        let mut placeholder_count = 0usize;
        let mut typed_placeholder_count = 0usize;

        for token in tokens {
            match token {
                Token::Literal(text) => {
                    literal_chars += text.chars().count();
                }
                Token::Placeholder { hint, .. } => {
                    placeholder_count += 1;
                    if hint.is_some() {
                        typed_placeholder_count += 1;
                    }
                }
                // Stray braces are treated as literal characters
                Token::OpenBrace { .. } | Token::CloseBrace { .. } => {
                    literal_chars += 1;
                }
            }
        }

        Ok(Self {
            literal_chars,
            placeholder_count,
            typed_placeholder_count,
        })
    }
}

impl Ord for SpecificityScore {
    fn cmp(&self, other: &Self) -> Ordering {
        // More literal characters → more specific
        match self.literal_chars.cmp(&other.literal_chars) {
            Ordering::Equal => {}
            ord => return ord,
        }

        // Fewer placeholders → more specific (reverse comparison)
        match other.placeholder_count.cmp(&self.placeholder_count) {
            Ordering::Equal => {}
            ord => return ord,
        }

        // More typed placeholders → more specific (tiebreaker)
        self.typed_placeholder_count
            .cmp(&other.typed_placeholder_count)
    }
}

impl PartialOrd for SpecificityScore {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Unwrap a result with a descriptive panic message on failure.
    fn score(pattern: &str) -> SpecificityScore {
        match SpecificityScore::calculate(pattern) {
            Ok(s) => s,
            Err(e) => panic!("pattern {pattern:?} should calculate successfully: {e}"),
        }
    }

    #[test]
    fn literal_only_pattern_has_highest_specificity() {
        let literal = score("overlap apples");
        let with_placeholder = score("overlap {item}");

        assert!(literal > with_placeholder);
        assert_eq!(literal.placeholder_count, 0);
        assert_eq!(with_placeholder.placeholder_count, 1);
    }

    #[test]
    fn more_literal_chars_wins() {
        let more_literal = score("the stdlib output is the workspace executable {path}");
        let less_literal = score("the stdlib output is {expected}");

        assert!(more_literal > less_literal);
    }

    #[test]
    fn fewer_placeholders_wins_with_equal_literals() {
        // Patterns with equal literal char counts but different placeholder counts
        let a = score("ab {x}");
        let b = score("a {x} {y}");

        assert_eq!(a.literal_chars, 3); // "ab "
        assert_eq!(b.literal_chars, 3); // "a " + " "
        assert!(a > b, "fewer placeholders should win when literals equal");
    }

    #[test]
    fn typed_placeholder_wins_as_tiebreaker() {
        let typed = score("count is {n:u32}");
        let untyped = score("count is {n}");

        assert_eq!(typed.literal_chars, untyped.literal_chars);
        assert_eq!(typed.placeholder_count, untyped.placeholder_count);
        assert!(
            typed > untyped,
            "typed placeholder should win as tiebreaker"
        );
    }

    #[test]
    fn empty_pattern_has_zero_specificity() {
        let empty = score("");

        assert_eq!(empty.literal_chars, 0);
        assert_eq!(empty.placeholder_count, 0);
        assert_eq!(empty.typed_placeholder_count, 0);
    }

    #[test]
    fn all_placeholder_pattern_has_lowest_specificity() {
        let all_placeholders = score("{a} {b} {c}");
        let mixed = score("prefix {a}");

        assert!(mixed > all_placeholders);
        assert_eq!(all_placeholders.literal_chars, 2); // two spaces
        assert_eq!(all_placeholders.placeholder_count, 3);
    }

    #[test]
    fn stray_braces_count_as_literal_chars() {
        let with_stray = score("{ literal }");

        // "{ literal }" tokenises as OpenBrace + Literal(" literal ") + CloseBrace
        assert_eq!(with_stray.literal_chars, 11); // 1 + 9 + 1
        assert_eq!(with_stray.placeholder_count, 0);
    }

    #[test]
    fn escaped_braces_count_as_literals() {
        let escaped = score("value is {{x}}");

        // "{{" becomes literal "{" and "}}" becomes literal "}"
        assert_eq!(escaped.literal_chars, 12); // "value is {x}"
        assert_eq!(escaped.placeholder_count, 0);
    }

    #[test]
    fn multibyte_characters_counted_correctly() {
        let unicode = score("café {value}");

        // "café " is 5 characters (not 6 bytes)
        assert_eq!(unicode.literal_chars, 5);
        assert_eq!(unicode.placeholder_count, 1);
    }

    #[test]
    fn real_world_example_from_issue() {
        // From issue #350: workspace executable pattern should beat generic
        let specific = score("the stdlib output is the workspace executable {path}");
        let generic = score("the stdlib output is {expected}");

        assert!(
            specific > generic,
            "workspace executable pattern ({} literals) should beat generic ({} literals)",
            specific.literal_chars,
            generic.literal_chars
        );
    }
}
