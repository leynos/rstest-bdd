//! Internal unit tests for the pattern scanner helpers.
//! These tests validate the small predicates and parsing functions introduced
//! during the refactor of `build_regex_from_pattern`. Keeping them here ensures
//! behaviour remains stable while allowing private access from a child module.

use crate::placeholder::{
    RegexBuilder, is_double_brace, is_empty_type_hint, is_escaped_brace, is_placeholder_start,
    parse_double_brace, parse_escaped_brace, parse_literal, parse_placeholder,
    parse_placeholder_name,
};

#[test]
fn predicates_detect_expected_tokens() {
    let s = br"\{\}{{}}{a}{_}";
    // Escaped braces
    assert!(is_escaped_brace(s, 0));
    assert!(!is_escaped_brace(s, 1));
    assert!(is_escaped_brace(s, 2));
    // Double braces
    assert!(is_double_brace(s, 4)); // "{{"
    assert!(is_double_brace(s, 6)); // "}}"
    // Placeholder start
    assert!(is_placeholder_start(s, 8)); // "{a"
    assert!(is_placeholder_start(s, 11)); // "{_"
}

#[test]
fn empty_type_hint_is_detected() {
    let pat = "{n:   }";
    let st = RegexBuilder::new(pat);
    // name_end just after "n" (index 2)
    let (name_end, _name) = parse_placeholder_name(&st, 1);
    assert!(is_empty_type_hint(&st, name_end));
}

#[test]
fn parse_escaped_and_double_braces() {
    // Escaped brace
    let mut st = RegexBuilder::new(r"\{");
    parse_escaped_brace(&mut st);
    assert_eq!(st.position, 2);
    assert!(st.output.ends_with(r"\{"));

    // Double brace
    let mut st2 = RegexBuilder::new("{{");
    parse_double_brace(&mut st2);
    assert_eq!(st2.position, 2);
    assert!(st2.output.ends_with(r"\{"));
}

#[test]
fn parse_placeholder_without_type_and_with_type() {
    // Without type; nested braces in placeholder content
    let mut st = RegexBuilder::new("before {outer {inner}} after");
    // Advance to the '{'
    st.position = "before ".len();
    #[expect(clippy::expect_used, reason = "test helper should fail loudly")]
    parse_placeholder(&mut st).expect("placeholder should parse");
    assert!(st.output.contains("(.+?)"));

    // With integer type
    let mut st2 = RegexBuilder::new("x {n:u32} y");
    st2.position = 2; // at '{'
    #[expect(clippy::expect_used, reason = "test helper should fail loudly")]
    parse_placeholder(&mut st2).expect("placeholder should parse");
    assert!(st2.output.contains(r"(\d+)"));
}

#[test]
fn parse_literal_writes_char() {
    let mut st = RegexBuilder::new("a");
    parse_literal(&mut st);
    assert_eq!(st.position, 1);
    assert!(st.output.ends_with('a'));
}
