//! Unit tests for the stderr normalizer pipeline.

use std::{borrow::Cow, panic};

use camino::Utf8Path;
use rstest::rstest;

use super::{
    super::{
        Normalizer,
        NormalizerInput,
        apply_normalizers,
        normalize_fixture_paths,
        run_compile_fail_with_normalized_output,
        strip_nightly_macro_backtrace_hint,
        wrappers::{FixtureStderr, FixtureTestPath},
    },
    NormalizerFixture,
};

#[test]
fn apply_normalizers_returns_borrowed_when_empty() {
    let result = apply_normalizers(NormalizerInput::from("message"), &[]);
    assert!(matches!(result, Cow::Borrowed("message")));
}

#[test]
fn apply_normalizers_respects_normalizer_order() {
    let add_prefix: Normalizer = |input| format!("prefix-{}", input.as_ref());
    let add_suffix: Normalizer = |input| format!("{}-suffix", input.as_ref());
    let result = apply_normalizers(NormalizerInput::from("value"), &[add_prefix, add_suffix]);
    assert_eq!(result, "prefix-value-suffix");
}

#[test]
fn apply_normalizers_handles_empty_string() {
    let trim_whitespace: Normalizer = |input| input.as_ref().trim().to_owned();
    let result = apply_normalizers(NormalizerInput::from(""), &[trim_whitespace]);
    assert_eq!(result, "");
}

#[test]
fn apply_normalizers_handles_whitespace_only_string() {
    let trim_whitespace: Normalizer = |input| input.as_ref().trim().to_owned();
    let mut whitespace = String::from("   ");
    whitespace.push('\n');
    let result = apply_normalizers(
        NormalizerInput::from(whitespace.as_str()),
        &[trim_whitespace],
    );
    assert_eq!(result, "");
}

#[test]
fn strip_nightly_macro_backtrace_hint_removes_multiple_hints() {
    let hint = " (in Nightly builds, run with -Z macro-backtrace for more info)";
    let text = format!("error: failure{hint} more context{hint}");
    let expected = "error: failure more context";
    assert_eq!(
        strip_nightly_macro_backtrace_hint(NormalizerInput::from(text.as_str())),
        expected
    );
}

#[test]
fn strip_nightly_macro_backtrace_hint_leaves_text_without_hint() {
    let text = "error: failure";
    assert_eq!(
        strip_nightly_macro_backtrace_hint(NormalizerInput::from(text)),
        text
    );
}

#[test]
fn normalize_fixture_paths_rewrites_relative_fixture_paths() {
    let dollar = '$';
    let input = "Warning:  --> tests/fixtures_macros/example.rs:3:1";
    let expected = format!("Warning:  --> {dollar}DIR/example.rs:3:1");
    assert_eq!(
        normalize_fixture_paths(NormalizerInput::from(input)),
        expected
    );
}

#[test]
fn normalize_fixture_paths_rewrites_absolute_fixture_paths() {
    let dollar = '$';
    let newline = '\n';
    let input = format!(
        " --> /tmp/workspace/crates/rstest-bdd/tests/fixtures_macros/example.rs:4:2{newline}"
    );
    let expected = format!(" --> {dollar}DIR/example.rs:4:2{newline}");
    assert_eq!(
        normalize_fixture_paths(NormalizerInput::from(input.as_ref())),
        expected
    );
}

#[test]
fn normalize_fixture_paths_is_idempotent_for_normalized_input() {
    let dollar = '$';
    let input = format!(" --> {dollar}DIR/example.rs:4:2");
    assert_eq!(
        normalize_fixture_paths(NormalizerInput::from(input.as_ref())),
        input
    );
}

#[test]
fn run_compile_fail_with_normalized_output_handles_multiple_normalizers() {
    const TEST_PATH: &str = "tests/fixtures_macros/__normaliser_multiple.rs";
    let mut expected = String::from("error: missing step (hint-one)");
    expected.push('\n');
    expected.push_str("help: review scenario (hint-two)");
    expected.push('\n');
    let mut actual = String::from("error: missing step");
    actual.push('\n');
    actual.push_str("help: review scenario");
    actual.push('\n');
    let fixture = NormalizerFixture::new(
        FixtureTestPath(TEST_PATH),
        FixtureStderr(expected.as_ref()),
        FixtureStderr(actual.as_ref()),
    );
    let strip_hint_one: Normalizer = |input| input.as_ref().replace(" (hint-one)", "");
    let strip_hint_two: Normalizer = |input| input.as_ref().replace(" (hint-two)", "");
    let result = panic::catch_unwind(|| {
        run_compile_fail_with_normalized_output(
            || panic!("expected failure"),
            Utf8Path::new(TEST_PATH),
            &[strip_hint_one, strip_hint_two],
        );
    });
    assert!(result.is_ok(), "normalized outputs should match");
    assert!(
        !fixture.actual_path.exists(),
        "successful normalization should delete the wip stderr file",
    );
}

/// A staged expected/actual stderr pair plus the outcome it should produce.
#[derive(Clone, Copy)]
struct NormalizerCase<'a> {
    /// Contents written to the expected stderr file.
    expected_content: &'a str,
    /// Contents written to the wip stderr file.
    actual_content: &'a str,
    /// Whether normalization is expected to accept the pair.
    should_succeed: bool,
    /// Message reported when the outcome does not match.
    result_message: &'a str,
    /// Message reported when the wip file is not in the expected state.
    file_message: &'a str,
}

#[test]
fn run_compile_fail_with_normalized_output_accepts_empty_output() {
    const TEST_PATH: &str = "tests/fixtures_macros/__normaliser_empty.rs";
    let fixture = NormalizerFixture::new(
        FixtureTestPath(TEST_PATH),
        FixtureStderr(""),
        FixtureStderr(""),
    );
    let result = panic::catch_unwind(|| {
        run_compile_fail_with_normalized_output(
            || panic!("expected failure"),
            Utf8Path::new(TEST_PATH),
            &[],
        );
    });
    assert!(result.is_ok(), "identical empty outputs should be accepted");
    assert!(
        !fixture.actual_path.exists(),
        "matching outputs should delete the wip stderr file",
    );
}

#[test]
fn run_compile_fail_with_normalized_output_detects_mismatch() {
    const TEST_PATH: &str = "tests/fixtures_macros/__normaliser_unexpected_detect.rs";
    let fixture = NormalizerFixture::new(
        FixtureTestPath(TEST_PATH),
        FixtureStderr("expected output"),
        FixtureStderr("actual output"),
    );
    let trim_trailing: Normalizer = |input| input.as_ref().trim_end().to_owned();
    let result = panic::catch_unwind(|| {
        run_compile_fail_with_normalized_output(
            || panic!("expected failure"),
            Utf8Path::new(TEST_PATH),
            &[trim_trailing],
        );
    });
    assert!(
        result.is_err(),
        "mismatched outputs must propagate the panic"
    );
    assert!(
        fixture.actual_path.exists(),
        "mismatched outputs should retain the wip stderr file for inspection",
    );
}

#[rstest]
#[case(
    "tests/fixtures_macros/__normaliser_whitespace.rs",
    NormalizerCase {
        expected_content: "warning: trailing space",
        actual_content: "warning: trailing space   ",
        should_succeed: true,
        result_message: "whitespace differences should be normalized",
        file_message: "matching outputs should delete the wip stderr file",
    }
)]
#[case(
    "tests/fixtures_macros/__normaliser_unexpected_case.rs",
    NormalizerCase {
        expected_content: "error: expected formatting",
        actual_content: "error: unexpected formatting",
        should_succeed: false,
        result_message: "mismatched outputs must propagate the panic",
        file_message: "mismatched outputs should retain the wip stderr file for inspection",
    }
)]
fn run_compile_fail_with_normalized_output_test_cases(
    #[case] test_path: &str,
    #[case] case: NormalizerCase<'_>,
) {
    let NormalizerCase {
        expected_content,
        actual_content,
        should_succeed,
        result_message,
        file_message,
    } = case;
    let mut expected = String::from(expected_content);
    expected.push('\n');
    let mut actual = String::from(actual_content);
    actual.push('\n');
    let fixture = NormalizerFixture::new(
        FixtureTestPath(test_path),
        FixtureStderr(expected.as_ref()),
        FixtureStderr(actual.as_ref()),
    );
    let trim_trailing: Normalizer = |input| input.as_ref().trim_end().to_owned();
    let result = panic::catch_unwind(|| {
        run_compile_fail_with_normalized_output(
            || panic!("expected failure"),
            Utf8Path::new(test_path),
            &[trim_trailing],
        );
    });

    if should_succeed {
        assert!(result.is_ok(), "{}", result_message);
        assert!(!fixture.actual_path.exists(), "{}", file_message);
    } else {
        assert!(result.is_err(), "{}", result_message);
        assert!(fixture.actual_path.exists(), "{}", file_message);
    }
}
