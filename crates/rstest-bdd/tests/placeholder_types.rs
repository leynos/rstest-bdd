//! Tests for additional placeholder patterns

use rstest_bdd::{PatternStr, StepText, extract_placeholders};

#[test]
fn parses_float_and_bool_placeholders() {
    let pattern = PatternStr::from("pi is {pi:f64} and flag is {flag:bool}");
    let text = StepText::from("pi is 3.14 and flag is true");
    let values =
        extract_placeholders(pattern, text).unwrap_or_else(|| panic!("expected placeholders"));
    assert_eq!(values, vec!["3.14".to_string(), "true".to_string()]);

    assert!(extract_placeholders(pattern, StepText::from("pi is x and flag is true")).is_none());
    assert!(
        extract_placeholders(pattern, StepText::from("pi is 3.14 and flag is maybe")).is_none()
    );
}

#[test]
fn matches_escaped_braces() {
    let pattern = PatternStr::from("set {{var}} to {value:u32}");
    let text = StepText::from("set {var} to 7");
    let values =
        extract_placeholders(pattern, text).unwrap_or_else(|| panic!("expected placeholders"));
    assert_eq!(values, vec!["7".to_string()]);
}
