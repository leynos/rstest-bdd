//! Tests for placeholder extraction logic.

use rstest_bdd::{PatternStr, StepText, extract_placeholders};

#[test]
fn type_hint_uses_specialised_fragment() {
    let pat = PatternStr::from("value {n:u32}");
    let text = StepText::from("value 42");
    let Some(caps) = extract_placeholders(pat, text) else {
        panic!("match expected");
    };
    assert_eq!(caps, vec!["42"]);
    assert!(
        extract_placeholders(pat, StepText::from("value none")).is_none(),
        "non-numeric text should not match"
    );
}

#[test]
fn handles_escaped_braces() {
    let pat = PatternStr::from(r"literal \{ brace {v} \}");
    let text = StepText::from("literal { brace data }");
    let Some(caps) = extract_placeholders(pat, text) else {
        panic!("match expected");
    };
    assert_eq!(caps, vec!["data"]);
}

#[test]
fn handles_nested_braces() {
    let pat = PatternStr::from("before {outer {inner}} after");
    let text = StepText::from("before value after");
    let Some(caps) = extract_placeholders(pat, text) else {
        panic!("match expected");
    };
    assert_eq!(caps, vec!["value"]);
}
