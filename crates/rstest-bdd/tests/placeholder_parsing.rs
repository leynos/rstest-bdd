//! Tests for placeholder extraction logic.

use rstest::rstest;
use rstest_bdd::localization::{strip_directional_isolates, ScopedLocalization};
use rstest_bdd::{
    extract_placeholders, PlaceholderError, PlaceholderSyntaxError, StepPattern, StepPatternError,
    StepText,
};
use std::borrow::Cow;
use std::ptr;
use unic_langid::langid;

#[expect(clippy::expect_used, reason = "test helper should fail loudly")]
fn compiled(pattern: &'static str) -> StepPattern {
    let pat = StepPattern::from(pattern);
    pat.compile().expect("failed to compile pattern");
    pat
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "test helper consumes the placeholder pattern by value"
)]
fn expect_placeholder_syntax(pat: StepPattern) -> PlaceholderSyntaxError {
    match pat.compile() {
        Err(StepPatternError::PlaceholderSyntax(e)) => e,
        other => panic!("expected PlaceholderSyntax error, got {other:?}"),
    }
}

#[test]
fn regex_requires_prior_compilation_and_caches() {
    let pattern = StepPattern::from("literal text");
    assert!(
        matches!(pattern.regex(), Err(StepPatternError::NotCompiled { .. })),
        "accessing the regex without compiling should return an error",
    );

    if let Err(err) = pattern.compile() {
        panic!("compiling literal pattern should succeed: {err:?}");
    }
    let re1 = match pattern.regex() {
        Ok(regex) => regex,
        Err(err) => panic!("regex should be available after compilation: {err:?}"),
    };
    assert!(re1.is_match("literal text"));

    let re2 = match pattern.regex() {
        Ok(regex) => regex,
        Err(err) => panic!("regex should be cached after compilation: {err:?}"),
    };
    assert!(re2.is_match("literal text"));
    assert!(
        ptr::eq(re1, re2),
        "repeated calls should return the cached regex instance",
    );
}

#[test]
fn regex_remains_unavailable_after_failed_compilation() {
    let pattern = StepPattern::from("value {n:}");

    assert!(
        pattern.compile().is_err(),
        "compile should fail for invalid pattern"
    );
    assert!(
        matches!(pattern.regex(), Err(StepPatternError::NotCompiled { .. })),
        "failed compilation should not populate the cached regex",
    );
}

#[test]
fn placeholder_error_reports_not_compiled() {
    let err = PlaceholderError::from(StepPatternError::NotCompiled {
        pattern: Cow::Borrowed("example"),
    });
    let PlaceholderError::NotCompiled { ref pattern } = err else {
        panic!("expected not compiled placeholder error");
    };
    assert_eq!(pattern, "example");
    assert_eq!(
        strip_directional_isolates(&err.to_string()),
        "step pattern 'example' must be compiled before use",
    );
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
    let err = expect_placeholder_syntax(StepPattern::from("value {n:}"));
    assert_eq!(err.position, 6);
    assert_eq!(err.placeholder.as_deref(), Some("n"));

    // Whitespace between the name and colon also produces an error.
    expect_placeholder_syntax(StepPattern::from("value {n : f64}"));

    // Whitespace immediately after the colon is invalid.
    expect_placeholder_syntax(StepPattern::from("value {n: f64}"));

    // Trailing whitespace before the closing brace is invalid.
    expect_placeholder_syntax(StepPattern::from("value {n:f64 }"));

    // Whitespace on both sides of the type hint is invalid.
    expect_placeholder_syntax(StepPattern::from("value {n: f64 }"));
}

#[test]
fn whitespace_before_closing_brace_is_error() {
    for pattern in ["value {n }", "value {n   }"] {
        let err = expect_placeholder_syntax(StepPattern::from(pattern));
        assert_eq!(err.position, 6);
        assert_eq!(err.placeholder.as_deref(), Some("n"));
    }
}

#[test]
fn extraction_reports_invalid_placeholder_error() {
    let pat = StepPattern::from("value {n:}");
    #[expect(clippy::expect_used, reason = "test asserts error variant")]
    let err = extract_placeholders(&pat, StepText::from("value 1"))
        .expect_err("placeholder error expected");
    assert!(matches!(err, PlaceholderError::InvalidPlaceholder(_)));
    assert_eq!(
        strip_directional_isolates(&err.to_string()),
        "invalid placeholder syntax: invalid placeholder in step pattern at byte 6 (zero-based) for placeholder 'n'",
    );
}

#[test]
fn invalid_pattern_error_display() {
    #[expect(
        clippy::invalid_regex,
        clippy::expect_used,
        reason = "deliberate invalid regex to test error display"
    )]
    let regex_err = regex::Regex::new("(").expect_err("invalid regex should error");
    let expected = format!("invalid step pattern: {regex_err}");
    let err: PlaceholderError = StepPatternError::from(regex_err).into();
    assert!(matches!(err, PlaceholderError::InvalidPattern(_)));
    assert_eq!(strip_directional_isolates(&err.to_string()), expected);
}

#[test]
fn placeholder_error_display_in_french() {
    let guard = ScopedLocalization::new(&[langid!("fr")])
        .unwrap_or_else(|error| panic!("failed to scope French locale: {error}"));
    let pat = StepPattern::from("value {n:}");
    #[expect(clippy::expect_used, reason = "test asserts error variant")]
    let err = extract_placeholders(&pat, StepText::from("value 1"))
        .expect_err("placeholder error expected");
    let display = strip_directional_isolates(&err.to_string());
    assert!(display.contains("syntaxe de paramètre invalide"));
    drop(guard);
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
    let pat = StepPattern::from("before {outer {inner}} after");
    pat.compile()
        .unwrap_or_else(|e| panic!("Failed to compile pattern: {e}"));
    let text = StepText::from("before value after");
    let Ok(caps) = extract_placeholders(&pat, text) else {
        panic!("match expected");
    };
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
