use super::wrappers::{FixtureStderr, FixtureTestPath};
use super::*;
use super::{Normaliser, NormaliserInput};
use rstest::rstest;
use std::borrow::Cow;
use std::fs;
use std::panic;
use std::path::{Path, PathBuf};

struct NormaliserFixture {
    expected_path: PathBuf,
    actual_path: PathBuf,
}

impl NormaliserFixture {
    fn new(
        test_path: FixtureTestPath<'_>,
        expected: FixtureStderr<'_>,
        actual: FixtureStderr<'_>,
    ) -> Self {
        let test_path = Path::new(test_path.as_ref());

        let expected_path = expected_stderr_path(test_path);
        if let Some(parent) = expected_path.parent() {
            fs::create_dir_all(parent).unwrap_or_else(|error| {
                panic!("failed to create directory for expected stderr fixture: {error}");
            });
        }
        fs::write(&expected_path, expected.as_ref()).unwrap_or_else(|error| {
            panic!("failed to write expected stderr fixture: {error}");
        });

        let actual_path = wip_stderr_path(test_path);
        if let Some(parent) = actual_path.parent() {
            fs::create_dir_all(parent).unwrap_or_else(|error| {
                panic!("failed to create directory for wip stderr fixture: {error}");
            });
        }
        fs::write(&actual_path, actual.as_ref()).unwrap_or_else(|error| {
            panic!("failed to write wip stderr fixture: {error}");
        });

        Self {
            expected_path,
            actual_path,
        }
    }
}

impl Drop for NormaliserFixture {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.expected_path);
        let _ = fs::remove_file(&self.actual_path);
    }
}

#[test]
fn wip_stderr_path_builds_target_location() {
    let path = wip_stderr_path(Path::new("tests/fixtures_macros/__helper_case.rs"));
    assert_eq!(path, Path::new("target/tests/wip/__helper_case.stderr"));
}

#[test]
#[should_panic(expected = "trybuild test path must include file name")]
fn wip_stderr_path_panics_without_file_name() {
    wip_stderr_path(Path::new(""));
}

#[test]
fn expected_stderr_path_replaces_extension() {
    let path = expected_stderr_path(Path::new("tests/ui_macros/example.output"));
    assert_eq!(path, Path::new("tests/ui_macros/example.stderr"));
}

#[test]
fn expected_stderr_path_handles_multiple_extensions() {
    let path = expected_stderr_path(Path::new("tests/ui_macros/example.feature.rs"));
    assert_eq!(path, Path::new("tests/ui_macros/example.feature.stderr"));
}

#[test]
fn apply_normalisers_returns_borrowed_when_empty() {
    let result = apply_normalisers(NormaliserInput::from("message"), &[]);
    assert!(matches!(result, Cow::Borrowed("message")));
}

#[test]
fn apply_normalisers_respects_normaliser_order() {
    let add_prefix: Normaliser = |input| format!("prefix-{}", input.as_ref());
    let add_suffix: Normaliser = |input| format!("{}-suffix", input.as_ref());
    let result = apply_normalisers(NormaliserInput::from("value"), &[add_prefix, add_suffix]);
    assert_eq!(result, "prefix-value-suffix");
}

#[test]
fn apply_normalisers_handles_empty_string() {
    let trim_whitespace: Normaliser = |input| input.as_ref().trim().to_owned();
    let result = apply_normalisers(NormaliserInput::from(""), &[trim_whitespace]);
    assert_eq!(result, "");
}

#[test]
fn apply_normalisers_handles_whitespace_only_string() {
    let trim_whitespace: Normaliser = |input| input.as_ref().trim().to_owned();
    let mut whitespace = String::from("   ");
    whitespace.push(char::from(10));
    let result = apply_normalisers(
        NormaliserInput::from(whitespace.as_str()),
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
        strip_nightly_macro_backtrace_hint(NormaliserInput::from(text.as_str())),
        expected
    );
}

#[test]
fn strip_nightly_macro_backtrace_hint_leaves_text_without_hint() {
    let text = "error: failure";
    assert_eq!(
        strip_nightly_macro_backtrace_hint(NormaliserInput::from(text)),
        text
    );
}

#[test]
fn normalise_fixture_paths_rewrites_relative_fixture_paths() {
    let dollar = char::from(36);
    let input = "Warning:  --> tests/fixtures_macros/example.rs:3:1";
    let expected = format!("Warning:  --> {dollar}DIR/example.rs:3:1");
    assert_eq!(
        normalise_fixture_paths(NormaliserInput::from(input)),
        expected
    );
}

#[test]
fn normalise_fixture_paths_rewrites_absolute_fixture_paths() {
    let dollar = char::from(36);
    let newline = char::from(10);
    let input = format!(
        " --> /tmp/workspace/crates/rstest-bdd/tests/fixtures_macros/example.rs:4:2{newline}"
    );
    let expected = format!(" --> {dollar}DIR/example.rs:4:2{newline}");
    assert_eq!(
        normalise_fixture_paths(NormaliserInput::from(input.as_ref())),
        expected
    );
}

#[test]
fn normalise_fixture_paths_is_idempotent_for_normalised_input() {
    let dollar = char::from(36);
    let input = format!(" --> {dollar}DIR/example.rs:4:2");
    assert_eq!(
        normalise_fixture_paths(NormaliserInput::from(input.as_ref())),
        input
    );
}

#[test]
fn run_compile_fail_with_normalised_output_handles_multiple_normalisers() {
    const TEST_PATH: &str = "tests/fixtures_macros/__normaliser_multiple.rs";
    let mut expected = String::from("error: missing step (hint-one)");
    expected.push(char::from(10));
    expected.push_str("help: review scenario (hint-two)");
    expected.push(char::from(10));
    let mut actual = String::from("error: missing step");
    actual.push(char::from(10));
    actual.push_str("help: review scenario");
    actual.push(char::from(10));
    let fixture = NormaliserFixture::new(
        FixtureTestPath(TEST_PATH),
        FixtureStderr(expected.as_ref()),
        FixtureStderr(actual.as_ref()),
    );
    let strip_hint_one: Normaliser = |input| input.as_ref().replace(" (hint-one)", "");
    let strip_hint_two: Normaliser = |input| input.as_ref().replace(" (hint-two)", "");
    let result = panic::catch_unwind(|| {
        run_compile_fail_with_normalised_output(
            || panic!("expected failure"),
            Path::new(TEST_PATH),
            &[strip_hint_one, strip_hint_two],
        );
    });
    assert!(result.is_ok(), "normalised outputs should match");
    assert!(
        !fixture.actual_path.exists(),
        "successful normalisation should delete the wip stderr file",
    );
}

#[test]
fn run_compile_fail_with_normalised_output_accepts_empty_output() {
    const TEST_PATH: &str = "tests/fixtures_macros/__normaliser_empty.rs";
    let fixture = NormaliserFixture::new(
        FixtureTestPath(TEST_PATH),
        FixtureStderr(""),
        FixtureStderr(""),
    );
    let result = panic::catch_unwind(|| {
        run_compile_fail_with_normalised_output(
            || panic!("expected failure"),
            Path::new(TEST_PATH),
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
fn run_compile_fail_with_normalised_output_detects_mismatch() {
    const TEST_PATH: &str = "tests/fixtures_macros/__normaliser_unexpected_detect.rs";
    let fixture = NormaliserFixture::new(
        FixtureTestPath(TEST_PATH),
        FixtureStderr("expected output"),
        FixtureStderr("actual output"),
    );
    let trim_trailing: Normaliser = |input| input.as_ref().trim_end().to_owned();
    let result = panic::catch_unwind(|| {
        run_compile_fail_with_normalised_output(
            || panic!("expected failure"),
            Path::new(TEST_PATH),
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
    "warning: trailing space",
    "warning: trailing space   ",
    true,
    "whitespace differences should be normalised",
    "matching outputs should delete the wip stderr file"
)]
#[case(
    "tests/fixtures_macros/__normaliser_unexpected_case.rs",
    "error: expected formatting",
    "error: unexpected formatting",
    false,
    "mismatched outputs must propagate the panic",
    "mismatched outputs should retain the wip stderr file for inspection"
)]
fn run_compile_fail_with_normalised_output_test_cases(
    #[case] test_path: &str,
    #[case] expected_content: &str,
    #[case] actual_content: &str,
    #[case] should_succeed: bool,
    #[case] result_message: &str,
    #[case] file_message: &str,
) {
    let mut expected = String::from(expected_content);
    expected.push(char::from(10));
    let mut actual = String::from(actual_content);
    actual.push(char::from(10));
    let fixture = NormaliserFixture::new(
        FixtureTestPath(test_path),
        FixtureStderr(expected.as_ref()),
        FixtureStderr(actual.as_ref()),
    );
    let trim_trailing: Normaliser = |input| input.as_ref().trim_end().to_owned();
    let result = panic::catch_unwind(|| {
        run_compile_fail_with_normalised_output(
            || panic!("expected failure"),
            Path::new(test_path),
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
