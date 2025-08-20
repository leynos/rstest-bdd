//! Tests for placeholder extraction logic.

use rstest::rstest;
use rstest_bdd::{StepPattern, StepText, extract_placeholders};

#[test]
fn type_hint_uses_specialised_fragment() {
    // u32: positive integer
    let pat = StepPattern::from("value {n:u32}");
    pat.compile()
        .unwrap_or_else(|e| panic!("Failed to compile pattern: {e}"));
    let text = StepText::from("value 42");
    let Some(caps) = extract_placeholders(&pat, text) else {
        panic!("match expected for u32");
    };
    assert_eq!(caps, vec!["42"]);
    assert!(
        extract_placeholders(&pat, StepText::from("value none")).is_none(),
        "non-numeric text should not match u32",
    );

    // i32: negative integer
    let pat = StepPattern::from("value {n:i32}");
    pat.compile()
        .unwrap_or_else(|e| panic!("Failed to compile pattern: {e}"));
    let text = StepText::from("value -42");
    let Some(caps) = extract_placeholders(&pat, text) else {
        panic!("match expected for negative i32");
    };
    assert_eq!(caps, vec!["-42"]);
    assert!(
        extract_placeholders(&pat, StepText::from("value 42.5")).is_none(),
        "float should not match i32",
    );

    // isize: negative integer
    let pat = StepPattern::from("value {n:isize}");
    pat.compile()
        .unwrap_or_else(|e| panic!("Failed to compile pattern: {e}"));
    let text = StepText::from("value -7");
    let Some(caps) = extract_placeholders(&pat, text) else {
        panic!("match expected for negative isize");
    };
    assert_eq!(caps, vec!["-7"]);

    // f64: floating point
    let pat = StepPattern::from("value {n:f64}");
    pat.compile()
        .unwrap_or_else(|e| panic!("Failed to compile pattern: {e}"));
    let text = StepText::from("value 2.71828");
    let Some(caps) = extract_placeholders(&pat, text) else {
        panic!("match expected for f64");
    };
    assert_eq!(caps, vec!["2.71828"]);
    assert!(
        extract_placeholders(&pat, StepText::from("value none")).is_none(),
        "non-numeric text should not match f64",
    );
    assert!(
        extract_placeholders(&pat, StepText::from("value -0.001")).is_some(),
        "negative float should match f64",
    );
    for text in ["value .5", "value 42.", "value 1e3", "value -1E-9"] {
        assert!(
            extract_placeholders(&pat, StepText::from(text)).is_some(),
            "{text} should match f64",
        );
    }
}

#[rstest]
#[case("literal {{ brace {v} }}", "literal { brace data }", Some(vec!["data"]))]
#[case("brace: {{}}", "brace: {}", Some(vec![]))]
#[case("{{{{}}}}", "{{}}", Some(vec![]))]
#[case("{{{v}}}", "{data}", Some(vec!["data"]))]
fn test_brace_escaping_scenarios(
    #[case] pattern: &'static str,
    #[case] input: &'static str,
    #[case] expected: Option<Vec<&'static str>>,
) {
    // Scenarios ensure escaped braces are literal and placeholders still match.
    let pat = StepPattern::from(pattern);
    pat.compile()
        .unwrap_or_else(|e| panic!("Failed to compile pattern: {e}"));
    let caps = extract_placeholders(&pat, StepText::from(input));
    let expected_owned = expected.map(|v| v.into_iter().map(String::from).collect::<Vec<_>>());
    assert_eq!(caps, expected_owned);
}

#[test]
fn unbalanced_braces_are_literals() {
    let pat = StepPattern::from("before {outer {inner} after");
    pat.compile()
        .unwrap_or_else(|e| panic!("Failed to compile pattern: {e}"));
    assert!(
        extract_placeholders(&pat, StepText::from("before value after")).is_none(),
        "text without literal brace should not match",
    );
}

#[test]
fn nested_brace_in_placeholder_is_literal() {
    let pat = StepPattern::from("{outer:{inner}}");
    pat.compile()
        .unwrap_or_else(|e| panic!("Failed to compile pattern: {e}"));
    assert!(
        extract_placeholders(&pat, StepText::from("value}")).is_some(),
        "trailing brace should be matched literally",
    );
    assert!(
        extract_placeholders(&pat, StepText::from("value")).is_none(),
        "missing closing brace should not match",
    );
}
