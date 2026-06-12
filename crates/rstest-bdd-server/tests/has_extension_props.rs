//! Property-based tests for the canonical `has_extension` predicate.
//!
//! Verifies the invariants the handler dedup relies on: ASCII-case
//! insensitivity, rejection of differing extensions, and stable behaviour
//! for paths with no extension, multiple dots, or trailing dots.

use std::path::PathBuf;

use proptest::prelude::*;

use rstest_bdd_server::handlers::util::has_extension;

/// Strategy producing a plausible lowercase extension (letters only so case
/// permutation is meaningful).
fn extension() -> impl Strategy<Value = String> {
    "[a-z]{1,8}"
}

/// Strategy producing a file stem free of dots and path separators.
fn stem() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9_-]{1,12}"
}

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

proptest! {
    /// `has_extension` is invariant to ASCII-case permutations on both the
    /// path's extension and the queried extension.
    #[test]
    fn case_permutation_is_irrelevant(
        stem in stem(),
        ext in extension(),
        path_mask in proptest::collection::vec(any::<bool>(), 0..8),
        query_mask in proptest::collection::vec(any::<bool>(), 0..8),
    ) {
        let path = PathBuf::from(format!("{stem}.{}", permute_case(&ext, &path_mask)));
        let query = permute_case(&ext, &query_mask);
        prop_assert!(has_extension(&path, &query));
    }

    /// A path whose extension differs from the query returns `false`.
    #[test]
    fn differing_extension_is_rejected(
        stem in stem(),
        ext in extension(),
        other in extension(),
    ) {
        prop_assume!(!ext.eq_ignore_ascii_case(&other));
        let path = PathBuf::from(format!("{stem}.{ext}"));
        prop_assert!(!has_extension(&path, &other));
    }

    /// A path with no extension never matches any queried extension.
    #[test]
    fn no_extension_never_matches(stem in stem(), ext in extension()) {
        let path = PathBuf::from(stem);
        prop_assert!(!has_extension(&path, &ext));
    }

    /// With multiple dots, only the final component counts as the extension.
    #[test]
    fn multiple_dots_use_final_component(
        stem in stem(),
        middle in extension(),
        ext in extension(),
    ) {
        let path = PathBuf::from(format!("{stem}.{middle}.{ext}"));
        prop_assert!(has_extension(&path, &ext));
        if !middle.eq_ignore_ascii_case(&ext) {
            prop_assert!(!has_extension(&path, &middle));
        }
    }

    /// A trailing dot yields an empty extension, which never matches a
    /// non-empty query.
    #[test]
    fn trailing_dot_matches_nothing(stem in stem(), ext in extension()) {
        let path = PathBuf::from(format!("{stem}."));
        prop_assert!(!has_extension(&path, &ext));
    }
}
