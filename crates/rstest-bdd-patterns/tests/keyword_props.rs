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

/// Strategy pairing a keyword with a case-flip mask sized to its rendering.
///
/// The mask must cover every character of the keyword: `permute_case` forces
/// any position beyond the mask to lower case, so a mask shorter than the
/// longest keyword (`Given`, five characters) would leave the final character
/// permanently lower-cased and silently exclude an upper-case final character
/// from the "arbitrary ASCII-case permutation" the property claims to test.
fn keyword_with_case_mask() -> impl Strategy<Value = (StepKeyword, Vec<bool>)> {
    keyword().prop_flat_map(|kw| {
        let len = kw.as_str().chars().count();
        (Just(kw), proptest::collection::vec(any::<bool>(), len))
    })
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
        (kw, mask) in keyword_with_case_mask(),
        leading in "[ \\t]{0,3}",
        trailing in "[ \\t]{0,3}",
    ) {
        let permuted = permute_case(kw.as_str(), &mask);
        let input = format!("{leading}{permuted}{trailing}");
        prop_assert_eq!(StepKeyword::from_str(&input), Ok(kw));
    }

    /// A string that matches no keyword (after trimming and case folding)
    /// fails with a parse error carrying the trimmed input.
    ///
    /// Surrounding whitespace is generated independently of the core token so
    /// the assertion exercises the trimming path: when `leading`/`trailing`
    /// are non-empty the raw input differs from its trimmed form, verifying
    /// that `from_str` trims before building the error and that the error
    /// carries the trimmed value rather than the raw input.
    #[test]
    fn non_keyword_strings_fail_to_parse(
        core in "[a-zA-Z0-9_-]{0,12}",
        leading in "[ \\t]{0,3}",
        trailing in "[ \\t]{0,3}",
    ) {
        // The core token carries no whitespace, so trimming the padded input
        // yields exactly `core`.
        let is_keyword = ALL_KEYWORDS
            .iter()
            .any(|kw| core.eq_ignore_ascii_case(kw.as_str()));
        prop_assume!(!is_keyword);
        let input = format!("{leading}{core}{trailing}");
        prop_assert_eq!(
            StepKeyword::from_str(&input),
            Err(StepKeywordParseError(core.clone()))
        );
    }
}
