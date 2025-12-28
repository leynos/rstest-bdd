//! Tests for pattern utilities.

use super::*;
use rstest::rstest;
use syn::parse_quote;

#[rstest]
#[case("_param", "param")]
#[case("param", "param")]
#[case("__param", "_param")]
#[case("_", "")]
#[case("", "")]
fn normalize_param_name_cases(#[case] input: &str, #[case] expected: &str) {
    assert_eq!(normalize_param_name(input), expected);
}

#[rstest]
#[case(parse_quote!(_param), "param", true)]
#[case(parse_quote!(param), "param", true)]
#[case(parse_quote!(__param), "_param", true)]
#[case(parse_quote!(__param), "param", false)]
#[case(parse_quote!(_other), "param", false)]
fn ident_matches_normalized_cases(
    #[case] ident: Ident,
    #[case] header: &str,
    #[case] expected: bool,
) {
    assert_eq!(ident_matches_normalized(&ident, header), expected);
}

/// Helper to assert placeholder extraction for a single-placeholder pattern.
#[expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test asserts valid pattern"
)]
fn assert_single_placeholder(pattern: &str, expected_name: &str, expected_hint: Option<&str>) {
    let summary = placeholder_names(pattern).expect("valid pattern");
    assert_eq!(summary.ordered.len(), 1);
    assert_eq!(summary.ordered[0].name, expected_name);
    assert_eq!(summary.ordered[0].hint, expected_hint.map(String::from));
}

/// Test that single-placeholder patterns correctly extract name and hint.
#[rstest]
#[case::without_hint("{foo}", "foo", None)]
#[case::with_u32_hint("{foo:u32}", "foo", Some("u32"))]
#[case::with_string_hint("{args:string}", "args", Some("string"))]
fn placeholder_hint_extraction(
    #[case] pattern: &str,
    #[case] expected_name: &str,
    #[case] expected_hint: Option<&str>,
) {
    assert_single_placeholder(pattern, expected_name, expected_hint);
}

#[test]
#[expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test asserts valid pattern"
)]
fn multiple_placeholders_with_mixed_hints() {
    let summary = placeholder_names("given {name} has {count:u32} items").expect("valid pattern");
    assert_eq!(summary.ordered.len(), 2);
    assert_eq!(summary.ordered[0].name, "name");
    assert_eq!(summary.ordered[0].hint, None);
    assert_eq!(summary.ordered[1].name, "count");
    assert_eq!(summary.ordered[1].hint, Some("u32".to_string()));
}

#[test]
#[expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test asserts valid pattern"
)]
fn placeholder_hints_align_with_names_for_wrapper_config() {
    // This test verifies that hints extracted from PlaceholderSummary maintain
    // correct alignment with placeholder names when converted to separate vectors.
    // This pattern matches the extraction logic in macros/mod.rs.
    let summary =
        placeholder_names("user {name:string} has {count:u32} and {note}").expect("valid pattern");

    // Simulate the extraction done in macros/mod.rs for WrapperInputs
    let placeholder_names: Vec<_> = summary.ordered.iter().map(|info| &info.name).collect();
    let placeholder_hints: Vec<_> = summary.ordered.iter().map(|info| &info.hint).collect();

    // Verify alignment: each name maps to its corresponding hint
    assert_eq!(placeholder_names.len(), 3);
    assert_eq!(placeholder_hints.len(), 3);

    // First: {name:string}
    assert_eq!(placeholder_names[0], "name");
    assert_eq!(placeholder_hints[0], &Some("string".to_string()));

    // Second: {count:u32}
    assert_eq!(placeholder_names[1], "count");
    assert_eq!(placeholder_hints[1], &Some("u32".to_string()));

    // Third: {note} - no hint
    assert_eq!(placeholder_names[2], "note");
    assert_eq!(placeholder_hints[2], &None);
}
