//! Behavioural tests for the public pattern parsing surface.
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

    // Negative match assertions guard against false positives.
    assert!(
        !regex.is_match("Given twelve"),
        "Should not match non-numeric value"
    );
    assert!(!regex.is_match("Given"), "Should not match missing value");
    assert!(
        !regex.is_match("Given 12x"),
        "Should not match extra characters after number"
    );

    assert!(
        compile_regex_from_pattern("broken {").is_err(),
        "malformed pattern should fail to compile"
    );
}

#[test]
fn compile_regex_from_pattern_edge_cases() {
    // Multiple placeholders
    let regex = compile_regex_from_pattern("Add {a:u32} and {b:u32}").expect("should compile");
    assert!(regex.is_match("Add 1 and 2"));

    // Unsupported type falls back to a lazy capture
    let regex = compile_regex_from_pattern("Value is {x:unknown}")
        .expect("unknown type should fallback to lazy match");
    assert!(regex.is_match("Value is apples"));

    // Empty pattern
    let regex = compile_regex_from_pattern("").expect("empty pattern should compile");
    assert!(regex.is_match(""));

    // Special regex characters in pattern
    let price_pattern = format!(
        "Price is {symbol}{pattern}",
        symbol = '$',
        pattern = "{p:u32}"
    );
    let regex = compile_regex_from_pattern(&price_pattern).expect("should compile");
    let price_input = format!("Price is {symbol}{value}", symbol = '$', value = 42);
    assert!(regex.is_match(&price_input));

    // Placeholder at start and end
    let regex = compile_regex_from_pattern("{x:u32} plus {y:u32}").expect("should compile");
    assert!(regex.is_match("12 plus 34"));

    // Adjacent placeholders
    let regex = compile_regex_from_pattern("{x:u32}{y:u32}").expect("should compile");
    assert!(regex.is_match("1234"), "Should match two adjacent numbers");
    assert!(
        !regex.is_match("12 34"),
        "Should not match numbers separated by space"
    );
    assert!(
        !regex.is_match("abcd"),
        "Should not match non-numeric input"
    );

    // Pattern with only placeholder
    let regex = compile_regex_from_pattern("{x:u32}").expect("should compile");
    assert!(regex.is_match("99"));
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
