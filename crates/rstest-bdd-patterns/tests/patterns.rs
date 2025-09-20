use regex::Regex;

use rstest_bdd_patterns::{build_regex_from_pattern, extract_captured_values, get_type_pattern};

#[test]
fn builds_regex_and_extracts_values() {
    let Ok(regex_src) = build_regex_from_pattern("I have {count:u32} cukes") else {
        panic!("unexpected pattern error");
    };
    let Ok(regex) = Regex::new(&regex_src) else {
        panic!("failed to compile regex");
    };
    let Some(captures) = extract_captured_values(&regex, "I have 12 cukes") else {
        panic!("expected captures for test step");
    };
    assert_eq!(captures, vec!["12".to_string()]);
}

#[test]
fn exposes_placeholder_error_details() {
    let Err(err) = build_regex_from_pattern("{value:}") else {
        panic!("expected placeholder error");
    };
    let info = match err {
        rstest_bdd_patterns::PatternError::Placeholder(info) => info,
        rstest_bdd_patterns::PatternError::Regex(other) => {
            panic!("expected placeholder error, got regex error {other}")
        }
    };
    assert_eq!(info.placeholder.as_deref(), Some("value"));
    assert!(info.to_string().contains("value"));
}

#[test]
fn maps_unknown_type_hint_to_lazy_match() {
    assert_eq!(get_type_pattern(Some("Custom")), r".+?");
    assert_eq!(get_type_pattern(None), r".+?");
}
