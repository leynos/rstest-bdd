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
fn malformed_type_hint_is_literal() {
    // Empty type hint is treated literally rather than as a placeholder.
    let pat = compiled("value {n:}");
    assert!(
        extract_placeholders(&pat, StepText::from("value 123")).is_err(),
        "malformed type hint should not capture",
    );

    // Whitespace between the name and colon makes it a literal placeholder.
    let pat2 = compiled("value {n : f64}");
    assert!(
        extract_placeholders(&pat2, StepText::from("value 1.0")).is_err(),
        "whitespace before colon should make the placeholder literal",
    );
}

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
fn unbalanced_braces_are_literals() {
    let pat = compiled("before {outer {inner} after");
    assert!(
        extract_placeholders(&pat, StepText::from("before value after")).is_err(),
        "text without literal brace should not match",
    );
    #[expect(clippy::expect_used, reason = "test asserts exact match")]
    let caps = extract_placeholders(&pat, StepText::from("before {outer {inner} after"))
        .expect("literal braces should match exactly");
    assert!(caps.is_empty(), "no placeholders expected");
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
fn stray_closing_brace_does_not_block_placeholders() {
    let pat = compiled("end} with {n:u32}");
    #[expect(clippy::expect_used, reason = "test asserts placeholder match")]
    let caps = extract_placeholders(&pat, StepText::from("end} with 7"))
        .expect("should match despite stray closing brace");
    assert_eq!(caps, vec!["7"]);
}

#[test]
fn stray_opening_brace_blocks_placeholders() {
    let pat = compiled("start{ with {n:u32}");
    assert!(
        extract_placeholders(&pat, StepText::from("start{ with 8")).is_err(),
        "placeholder should not match after stray opening brace",
    );
}
