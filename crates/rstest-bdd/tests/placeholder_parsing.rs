//! Tests for placeholder extraction logic.

use rstest::rstest;
use rstest_bdd::{StepPattern, StepText, extract_placeholders};

#[expect(clippy::expect_used, reason = "test helper should fail loudly")]
fn compiled(pattern: &'static str) -> StepPattern {
    let pat = StepPattern::from(pattern);
    pat.compile().expect("failed to compile pattern");
    pat
}

#[test]
fn type_hint_uses_specialised_fragment() {
    // u32: positive integer
    let pat = compiled("value {n:u32}");
    let text = StepText::from("value 42");
    let Ok(caps) = extract_placeholders(&pat, text) else {
        panic!("match expected for u32");
    };
    assert_eq!(caps, vec!["42"]);
    assert!(
        extract_placeholders(&pat, StepText::from("value none")).is_err(),
        "non-numeric text should not match u32",
    );

    // i32: negative integer
    let pat = compiled("value {n:i32}");
    let text = StepText::from("value -42");
    let Ok(caps) = extract_placeholders(&pat, text) else {
        panic!("match expected for negative i32");
    };
    assert_eq!(caps, vec!["-42"]);
    assert!(
        extract_placeholders(&pat, StepText::from("value 42.5")).is_err(),
        "float should not match i32",
    );

    // isize: negative integer
    let pat = compiled("value {n:isize}");
    let text = StepText::from("value -7");
    let Ok(caps) = extract_placeholders(&pat, text) else {
        panic!("match expected for negative isize");
    };
    assert_eq!(caps, vec!["-7"]);

    // f64: floating point
    let pat = compiled("value {n:f64}");
    let text = StepText::from("value 2.71828");
    let Ok(caps) = extract_placeholders(&pat, text) else {
        panic!("match expected for f64");
    };
    assert_eq!(caps, vec!["2.71828"]);
    assert!(
        extract_placeholders(&pat, StepText::from("value none")).is_err(),
        "non-numeric text should not match f64",
    );
    assert!(
        extract_placeholders(&pat, StepText::from("value 1e-3")).is_ok(),
        "scientific notation should match f64",
    );
    assert!(
        extract_placeholders(&pat, StepText::from("value -0.001")).is_ok(),
        "negative float should match f64",
    );
    for sample in [
        "value .5",
        "value 42.",
        "value 1e3",
        "value -1E-9",
        "value -.5",
        "value +3.0",
        "value NaN",
        "value inf",
        "value Infinity",
    ] {
        assert!(
            extract_placeholders(&pat, StepText::from(sample)).is_ok(),
            "{sample} should match f64",
        );
    }

    // f32: special float values
    let pat = compiled("value {n:f32}");
    for sample in ["value NaN", "value inf", "value Infinity"] {
        assert!(
            extract_placeholders(&pat, StepText::from(sample)).is_ok(),
            "{sample} should match f32",
        );
    }
}

#[rstest]
#[case("value {n:foo}", "value anything", "anything")]
fn invalid_type_hint_is_generic(
    #[case] pattern: &'static str,
    #[case] input: &'static str,
    #[case] expected: &'static str,
) {
    // Unknown type hints fall back to a greedy match.
    let pat = compiled(pattern);
    #[expect(clippy::expect_used, reason = "test asserts placeholder match")]
    let caps = extract_placeholders(&pat, StepText::from(input))
        .expect("invalid type hint should still capture");
    assert_eq!(caps, vec![expected]);
}

#[test]
fn malformed_type_hint_is_error() {
    // Empty type hint now yields a compilation error.
    let pat = StepPattern::from("value {n:}");
    assert!(pat.compile().is_err(), "empty type hint should be invalid");

    // Whitespace between the name and colon also produces an error.
    let pat2 = StepPattern::from("value {n : f64}");
    assert!(
        pat2.compile().is_err(),
        "whitespace before colon is invalid"
    );
}

#[test]
fn handles_escaped_braces() {
    let pat = StepPattern::from(r"literal \{ brace {v} \}");
    pat.compile()
        .unwrap_or_else(|e| panic!("Failed to compile pattern: {e}"));
    let text = StepText::from("literal { brace data }");
    let Ok(caps) = extract_placeholders(&pat, text) else {
        panic!("match expected");
    };
    assert_eq!(caps, vec!["data"]);
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
    let pat = compiled(pattern);
    let caps = extract_placeholders(&pat, StepText::from(input)).ok();
    let expected_owned = expected.map(|v| v.into_iter().map(String::from).collect::<Vec<_>>());
    assert_eq!(caps, expected_owned);
}

#[test]
fn handles_nested_braces() {
    let pat = StepPattern::from("before {outer {inner}} after");
    pat.compile()
        .unwrap_or_else(|e| panic!("Failed to compile pattern: {e}"));
    let text = StepText::from("before value after");
    let Ok(caps) = extract_placeholders(&pat, text) else {
        panic!("match expected");
    };
    assert_eq!(caps, vec!["value"]);
}

#[test]
fn compile_fails_on_unbalanced_braces() {
    let pat = StepPattern::from("before {outer {inner} after");
    assert!(pat.compile().is_err(), "unbalanced braces should error");
}

#[test]
fn nested_brace_in_placeholder_is_literal() {
    let pat = compiled("{outer:{inner}}");
    assert!(
        extract_placeholders(&pat, StepText::from("value}")).is_ok(),
        "trailing brace should be matched literally",
    );
    assert!(
        extract_placeholders(&pat, StepText::from("value")).is_err(),
        "missing closing brace should not match",
    );
}

#[test]
fn compile_fails_on_stray_closing_brace() {
    let pat = StepPattern::from("end} with {n:u32}");
    assert!(pat.compile().is_err(), "stray closing brace should error");
}

#[test]
fn compile_fails_on_stray_opening_brace() {
    let pat = StepPattern::from("start{ with {n:u32}");
    assert!(pat.compile().is_err(), "stray opening brace should error");
}
