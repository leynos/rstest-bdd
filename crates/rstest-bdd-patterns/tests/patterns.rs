#![expect(clippy::expect_used, reason = "test asserts conversion path")]

use regex::Regex;

use rstest_bdd_patterns::{
    build_regex_from_pattern, compile_regex_from_pattern, extract_captured_values, get_type_pattern,
};

#[test]
fn compile_regex_from_pattern_smoke_test() {
    let regex =
        compile_regex_from_pattern("Given {n:u32}").expect("pattern should compile into regex");
    assert!(regex.is_match("Given 12"));

    assert!(
        compile_regex_from_pattern("broken {").is_err(),
        "malformed pattern should fail to compile"
    );
}

#[test]
fn builds_regex_and_extracts_values() {
    let regex_src =
        build_regex_from_pattern("I have {count:u32} cukes").expect("pattern should compile");
    let regex = Regex::new(&regex_src).expect("regex should compile");
    let captures = extract_captured_values(&regex, "I have 12 cukes")
        .expect("expected captures for test step");
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

#[test]
fn rejects_placeholder_hint_with_whitespace() {
    let Err(err) = build_regex_from_pattern("{value:bad hint}") else {
        panic!("expected placeholder error");
    };
    assert!(err.to_string().contains("invalid placeholder"));
}

#[test]
fn rejects_placeholder_hint_with_braces() {
    let Err(err) = build_regex_from_pattern("{value:Vec<{u32}>}") else {
        panic!("expected placeholder error");
    };
    assert!(err.to_string().contains("invalid placeholder"));
}
