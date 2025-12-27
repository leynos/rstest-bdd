//! Tests covering brace escaping and stray brace handling in step patterns.

mod support;

use rstest::rstest;
use rstest_bdd::{StepPattern, StepPatternError, StepText, extract_placeholders};
use support::{compile_and_extract, compiled, expect_placeholder_syntax};

#[test]
fn handles_escaped_braces() {
    let caps = compile_and_extract(r"literal \{ brace {v} \}", "literal { brace data }");
    assert_eq!(caps, vec!["data"]);
}

#[rstest]
#[case(r"digit \d end", "digit d end", "digit 5 end")]
#[case(r"hex \x end", "hex x end", "hex 7 end")]
#[case(r"quote \q end", r"quote q end", r#"quote " end"#)]
#[case(r"end \Z here", "end Z here", "end 0 here")]
#[case(r"back \\ slash", r"back \ slash", r"back \\ slash")]
fn unknown_escape_is_literal(
    #[case] pattern: &'static str,
    #[case] matching: &'static str,
    #[case] nonmatching: &'static str,
) {
    let pat = compiled(pattern);
    #[expect(clippy::expect_used, reason = "test asserts literal match")]
    let caps = extract_placeholders(&pat, StepText::from(matching))
        .expect("literal character should match");
    assert!(caps.is_empty(), "no placeholders expected");
    assert!(
        extract_placeholders(&pat, StepText::from(nonmatching)).is_err(),
        "escape should be treated literally",
    );
}

#[test]
fn trailing_backslash_is_literal() {
    // Use a normal string here; raw strings cannot end with a backslash.
    let pat = compiled("foo\\");
    #[expect(clippy::expect_used, reason = "test asserts literal match")]
    let caps = extract_placeholders(&pat, StepText::from("foo\\"))
        .expect("literal backslash should match");
    assert!(caps.is_empty(), "no placeholders expected");
    assert!(
        extract_placeholders(&pat, StepText::from("foo")).is_err(),
        "missing trailing backslash should not match",
    );
}

#[test]
fn unknown_escape_inside_stray_depth_is_literal() {
    // The opening "{" puts the scanner into stray-depth mode; "\d" must stay literal.
    let pat = compiled(r"start{ \d }end");
    #[expect(clippy::expect_used, reason = "test asserts literal match")]
    let caps = extract_placeholders(&pat, StepText::from(r"start{ d }end"))
        .expect("literal d should match inside stray depth");
    assert!(caps.is_empty(), "no placeholders expected");
    assert!(
        extract_placeholders(&pat, StepText::from(r"start{ 5 }end")).is_err(),
        "digit class must not be interpreted inside stray depth",
    );
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
    // The outer braces form stray text; the inner `{inner}` is a real placeholder.
    // This ensures the scanner handles nested brace pairs without mis-parsing placeholders.
    let caps = compile_and_extract("before {outer {inner}} after", "before value after");
    assert_eq!(caps, vec!["value"]);
}

#[rstest]
#[case(
    "before {outer {inner} after",
    "nested unbalanced opening brace should error"
)]
#[case(
    "{unbalanced start text",
    "unbalanced opening brace at start should error"
)]
#[case(
    "text with unbalanced end}",
    "unbalanced closing brace at end should error"
)]
#[case(
    "text {with {multiple unbalanced",
    "multiple unbalanced opening braces should error"
)]
#[case(
    "text} with} multiple unbalanced",
    "multiple unbalanced closing braces should error"
)]
#[case(
    "start {middle text} end}",
    "unbalanced closing brace in middle should error"
)]
fn compile_fails_on_unbalanced_braces(
    #[case] pattern: &'static str,
    #[case] description: &'static str,
) {
    let pat = StepPattern::from(pattern);
    assert!(
        matches!(pat.compile(), Err(StepPatternError::PlaceholderSyntax(_))),
        "{description}"
    );
}

#[test]
fn nested_brace_in_placeholder_is_error() {
    let err = expect_placeholder_syntax(StepPattern::from("{outer:{inner}}"));
    assert_eq!(err.position, 0);
    assert_eq!(err.placeholder.as_deref(), Some("outer"));
}

#[test]
fn compile_fails_on_stray_closing_brace() {
    expect_placeholder_syntax(StepPattern::from("end} with {n:u32}"));
}

#[test]
fn compile_fails_on_stray_opening_brace() {
    expect_placeholder_syntax(StepPattern::from("start{ with {n:u32}"));
}

#[test]
fn braces_in_type_hint_are_invalid() {
    let err = expect_placeholder_syntax(StepPattern::from("value {n:{u32}}"));
    assert_eq!(err.position, 6);
    assert_eq!(err.placeholder.as_deref(), Some("n"));

    let err2 = expect_placeholder_syntax(StepPattern::from("value {n:Vec<{u32}>}"));
    assert_eq!(err2.position, 6);
    assert_eq!(err2.placeholder.as_deref(), Some("n"));

    assert!(StepPattern::from("value {n:u32}").compile().is_ok());
    assert!(StepPattern::from("value {what:String}").compile().is_ok());
}
