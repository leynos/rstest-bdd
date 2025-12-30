//! Tests for placeholder extraction logic.

use rstest::rstest;
use rstest_bdd::localization::{ScopedLocalization, strip_directional_isolates};
use rstest_bdd::{PlaceholderError, StepPattern, StepPatternError, StepText, extract_placeholders};
use std::borrow::Cow;
use std::ptr;
use unic_langid::langid;

mod support;
use support::{compiled, expect_placeholder_syntax};

/// Anchor the helper so it is not flagged as dead code when compiling this test.
const _: fn(&'static str, &str) -> Vec<String> = support::compile_and_extract;

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
    // Unknown type hints fall back to a non-greedy match.
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
fn string_hint_captures_quoted_values() {
    // :string hint matches double-quoted strings
    let pat = compiled("message is {text:string}");
    let text = StepText::from(r#"message is "hello world""#);
    let Ok(caps) = extract_placeholders(&pat, text) else {
        panic!("match expected for :string with double quotes");
    };
    // Captured value includes quotes (stripping happens in generated code)
    assert_eq!(caps, vec![r#""hello world""#]);

    // :string hint matches single-quoted strings
    let text = StepText::from("message is 'hello world'");
    let Ok(caps) = extract_placeholders(&pat, text) else {
        panic!("match expected for :string with single quotes");
    };
    assert_eq!(caps, vec!["'hello world'"]);

    // :string hint does not match unquoted text
    assert!(
        extract_placeholders(&pat, StepText::from("message is hello world")).is_err(),
        "unquoted text should not match :string",
    );

    // :string hint matches empty quoted strings
    let text = StepText::from(r#"message is """#);
    let Ok(caps) = extract_placeholders(&pat, text) else {
        panic!("match expected for :string with empty double quotes");
    };
    assert_eq!(caps, vec![r#""""#]);
}

#[test]
fn string_hint_with_multiple_placeholders() {
    // Mixed placeholders: :string and :u32
    let pat = compiled("user {name:string} has {count:u32} items");
    let text = StepText::from(r#"user "Alice" has 5 items"#);
    let Ok(caps) = extract_placeholders(&pat, text) else {
        panic!("match expected for mixed :string and :u32");
    };
    assert_eq!(caps, vec![r#""Alice""#, "5"]);

    // Verify type constraints are enforced for each placeholder
    assert!(
        extract_placeholders(&pat, StepText::from("user Alice has 5 items")).is_err(),
        "unquoted name should not match :string",
    );
    assert!(
        extract_placeholders(&pat, StepText::from(r#"user "Alice" has five items"#)).is_err(),
        "non-numeric count should not match :u32",
    );
}
