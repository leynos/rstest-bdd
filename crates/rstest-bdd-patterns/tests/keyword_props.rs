//! Property-based tests for the canonical `StepKeyword` keyword table.
//!
//! Pins the round-trip contract between `as_str` (rendering) and `from_str`
//! (parsing): every keyword renders to a string that parses back to itself,
//! parsing is invariant to ASCII case and surrounding whitespace, and
//! non-keyword strings fail with a `StepKeywordParseError` carrying the
//! trimmed input.

use std::str::FromStr;

use proptest::prelude::*;

use rstest_bdd_patterns::{StepKeyword, StepKeywordParseError};

const ALL_KEYWORDS: [StepKeyword; 5] = [
    StepKeyword::Given,
    StepKeyword::When,
    StepKeyword::Then,
    StepKeyword::And,
    StepKeyword::But,
];

/// Apply a per-character case flip mask to an ASCII string.
fn permute_case(s: &str, mask: &[bool]) -> String {
    s.chars()
        .zip(mask.iter().copied().chain(std::iter::repeat(false)))
        .map(|(c, flip)| {
            if flip {
                c.to_ascii_uppercase()
            } else {
                c.to_ascii_lowercase()
            }
        })
        .collect()
}

/// Strategy selecting one of the five keyword variants.
fn keyword() -> impl Strategy<Value = StepKeyword> {
    proptest::sample::select(ALL_KEYWORDS.as_slice())
}

proptest! {
    /// Rendering then parsing returns the original keyword.
    #[test]
    fn round_trip_via_canonical_rendering(kw in keyword()) {
        prop_assert_eq!(StepKeyword::from_str(kw.as_str()), Ok(kw));
    }

    /// Parsing is invariant to ASCII case permutations and surrounding
    /// whitespace.
    #[test]
    fn parse_ignores_case_and_whitespace(
        kw in keyword(),
        mask in proptest::collection::vec(any::<bool>(), 0..5),
        leading in "[ \\t]{0,3}",
        trailing in "[ \\t]{0,3}",
    ) {
        let permuted = permute_case(kw.as_str(), &mask);
        let input = format!("{leading}{permuted}{trailing}");
        prop_assert_eq!(StepKeyword::from_str(&input), Ok(kw));
    }

    /// A string that matches no keyword (after trimming and case folding)
    /// fails with a parse error carrying the trimmed input.
    #[test]
    fn non_keyword_strings_fail_to_parse(input in "[a-zA-Z0-9_-]{0,12}") {
        let is_keyword = ALL_KEYWORDS
            .iter()
            .any(|kw| input.trim().eq_ignore_ascii_case(kw.as_str()));
        prop_assume!(!is_keyword);
        prop_assert_eq!(
            StepKeyword::from_str(&input),
            Err(StepKeywordParseError(input.trim().to_string()))
        );
    }
}
