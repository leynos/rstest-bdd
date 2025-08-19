//! Tests for placeholder extraction logic.

use rstest_bdd::{StepPattern, StepPatternCompileError, StepText, extract_placeholders};

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
}

#[test]
fn handles_escaped_braces() {
    let pat = StepPattern::from(r"literal \{ brace {v} \}");
    pat.compile()
        .unwrap_or_else(|e| panic!("Failed to compile pattern: {e}"));
    let text = StepText::from("literal { brace data }");
    let Some(caps) = extract_placeholders(&pat, text) else {
        panic!("match expected");
    };
    assert_eq!(caps, vec!["data"]);
}

#[test]
fn handles_nested_braces() {
    let pat = StepPattern::from("before {outer {inner}} after");
    pat.compile()
        .unwrap_or_else(|e| panic!("Failed to compile pattern: {e}"));
    let text = StepText::from("before value after");
    let Some(caps) = extract_placeholders(&pat, text) else {
        panic!("match expected");
    };
    assert_eq!(caps, vec!["value"]);
}

#[test]
fn unbalanced_braces_return_error() {
    let pat = StepPattern::from("before {outer {inner} after");
    let result = pat.compile();
    assert!(
        matches!(result, Err(StepPatternCompileError::UnbalancedBraces)),
        "unbalanced braces should return error",
    );
}
