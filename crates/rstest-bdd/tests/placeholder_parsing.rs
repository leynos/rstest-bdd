//! Tests for placeholder extraction logic.

use rstest::rstest;
use rstest_bdd::{PlaceholderError, StepPattern, StepPatternError, StepText, extract_placeholders};

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
fn malformed_type_hint_is_error() {
    // Empty type hint now yields a compilation error.
    let pat = StepPattern::from("value {n:}");
    assert!(
        matches!(pat.compile(), Err(StepPatternError::PlaceholderSyntax(_))),
        "empty type hint should be invalid"
    );

    // Whitespace between the name and colon also produces an error.
    let pat2 = StepPattern::from("value {n : f64}");
    assert!(
        matches!(pat2.compile(), Err(StepPatternError::PlaceholderSyntax(_))),
        "whitespace before colon is invalid"
    );

    // Whitespace immediately after the colon is invalid.
    let pat3 = StepPattern::from("value {n: f64}");
    assert!(
        matches!(pat3.compile(), Err(StepPatternError::PlaceholderSyntax(_))),
        "whitespace after colon is invalid"
    );

    // Trailing whitespace before the closing brace is invalid.
    let pat4 = StepPattern::from("value {n:f64 }");
    assert!(
        matches!(pat4.compile(), Err(StepPatternError::PlaceholderSyntax(_))),
        "whitespace around type hint is invalid"
    );
}

#[test]
fn whitespace_before_closing_brace_is_error() {
    for pattern in ["value {n }", "value {n   }"] {
        let pat = StepPattern::from(pattern);
        #[expect(clippy::expect_used, reason = "test asserts error")]
        let err = pat
            .compile()
            .expect_err("whitespace before closing brace should error");
        let StepPatternError::PlaceholderSyntax(e) = err else {
            panic!("unexpected error variant");
        };
        assert_eq!(e.position, 6);
        assert_eq!(e.placeholder.as_deref(), Some("n"));
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
        err.to_string(),
        "invalid placeholder syntax: invalid placeholder in step pattern at byte 6 (zero-based) for placeholder `n`"
    );
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
#[case("before {outer {inner} after")]
#[case("{unbalanced start text")]
#[case("text with unbalanced end}")]
#[case("text {with {multiple unbalanced")]
#[case("text} with} multiple unbalanced")]
#[case("start {middle text} end}")]
fn compile_fails_on_unbalanced_braces(#[case] pattern: &'static str) {
    let pat = StepPattern::from(pattern);
    assert!(
        matches!(pat.compile(), Err(StepPatternError::PlaceholderSyntax(_))),
        "unbalanced braces should error: {pattern}"
    );
}

#[test]
fn nested_brace_in_placeholder_is_error() {
    let pat = StepPattern::from("{outer:{inner}}");
    match pat.compile() {
        Err(StepPatternError::PlaceholderSyntax(err)) => {
            assert_eq!(err.position, 0);
            assert_eq!(err.placeholder.as_deref(), Some("outer"));
        }
        _ => panic!("nested brace in type hint should be invalid"),
    }
}

#[test]
fn compile_fails_on_stray_closing_brace() {
    let pat = StepPattern::from("end} with {n:u32}");
    assert!(
        matches!(pat.compile(), Err(StepPatternError::PlaceholderSyntax(_))),
        "stray closing brace should error"
    );
}

#[test]
fn compile_fails_on_stray_opening_brace() {
    let pat = StepPattern::from("start{ with {n:u32}");
    assert!(
        matches!(pat.compile(), Err(StepPatternError::PlaceholderSyntax(_))),
        "stray opening brace should error"
    );
}
#[test]
fn braces_in_type_hint_are_invalid() {
    let pat = StepPattern::from("value {n:{u32}}");
    match pat.compile() {
        Err(StepPatternError::PlaceholderSyntax(err)) => {
            assert_eq!(err.position, 6);
            assert_eq!(err.placeholder.as_deref(), Some("n"));
        }
        _ => panic!("brace in type hint should error"),
    }

    let pat2 = StepPattern::from("value {n:Vec<{u32}>}");
    match pat2.compile() {
        Err(StepPatternError::PlaceholderSyntax(err)) => {
            assert_eq!(err.position, 6);
            assert_eq!(err.placeholder.as_deref(), Some("n"));
        }
        _ => panic!("brace in nested type hint should error"),
    }

    assert!(StepPattern::from("value {n:u32}").compile().is_ok());
    assert!(StepPattern::from("value {what:String}").compile().is_ok());
}
