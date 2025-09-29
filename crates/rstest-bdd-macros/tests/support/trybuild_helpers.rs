use rstest::rstest;
use std::borrow::Cow;
use std::fs;
use std::panic;
use std::path::{Path, PathBuf};

use super::support::{
    FixturePath, Normaliser, apply_normalisers, normalise_fixture_paths,
    run_compile_fail_with_normalised_output, strip_nightly_macro_backtrace_hint,
};

struct NormaliserFixture {
    expected_path: PathBuf,
    actual_path: PathBuf,
}

impl NormaliserFixture {
    fn new(test_path: FixturePath<'_>, expected: &str, actual: &str) -> Self {
        let expected_path = test_path.expected_stderr_path();
        if let Some(parent) = expected_path.parent() {
            if let Err(error) = fs::create_dir_all(parent) {
                panic!("failed to create directory for expected stderr fixture: {error}");
            }
        }
        if let Err(error) = fs::write(&expected_path, expected) {
            panic!("failed to write expected stderr fixture: {error}");
        }

        let actual_path = test_path.wip_stderr_path();
        if let Some(parent) = actual_path.parent() {
            if let Err(error) = fs::create_dir_all(parent) {
                panic!("failed to create directory for wip stderr fixture: {error}");
            }
        }
        if let Err(error) = fs::write(&actual_path, actual) {
            panic!("failed to write wip stderr fixture: {error}");
        }

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
    let path = FixturePath::new("tests/fixtures/__helper_case.rs").wip_stderr_path();
    assert_eq!(path, Path::new("target/tests/wip/__helper_case.stderr"));
}

#[test]
#[should_panic(expected = "trybuild test path must include file name")]
fn wip_stderr_path_panics_without_file_name() {
    let _ = FixturePath::new("").wip_stderr_path();
}

#[rstest]
#[case::single_extension(FixturePath::new("tests/ui/example.output"), "tests/ui/example.stderr")]
#[case::multiple_extensions(
    FixturePath::new("tests/ui/example.feature.rs"),
    "tests/ui/example.feature.stderr"
)]
fn expected_stderr_path_rewrites_extension(
    #[case] input: FixturePath<'static>,
    #[case] expected: &str,
) {
    let path = input.expected_stderr_path();
    assert_eq!(path, Path::new(expected));
}

#[test]
fn apply_normalisers_returns_borrowed_when_empty() {
    let result = apply_normalisers("message", &[]);
    assert!(matches!(result, Cow::Borrowed("message")));
}

#[test]
fn apply_normalisers_respects_normaliser_order() {
    let add_prefix: Normaliser = |text| format!("prefix-{text}");
    let add_suffix: Normaliser = |text| format!("{text}-suffix");
    let result = apply_normalisers("value", &[add_prefix, add_suffix]);
    assert_eq!(result, "prefix-value-suffix");
}

#[rstest]
#[case::empty(String::new(), "")]
#[case::whitespace({
    let mut text = String::from("   ");
    text.push(char::from(10));
    text
}, "")]
fn apply_normalisers_trims_whitespace(#[case] input: String, #[case] expected: &str) {
    let trim_whitespace: Normaliser = |text| text.trim().to_owned();
    let result = apply_normalisers(input.as_str(), &[trim_whitespace]);
    assert_eq!(result, expected);
}

#[rstest]
#[case::removes_multiple(
    concat!(
        "error: failure",
        " (in Nightly builds, run with -Z macro-backtrace for more info)",
        " more context",
        " (in Nightly builds, run with -Z macro-backtrace for more info)"
    ),
    "error: failure more context"
)]
#[case::leaves_text_unchanged("error: failure", "error: failure")]
fn strip_nightly_macro_backtrace_hint_cases(#[case] input: &str, #[case] expected: &str) {
    assert_eq!(strip_nightly_macro_backtrace_hint(input), expected);
}

#[rstest]
#[case::relative(
    Cow::from("Warning:  --> tests/fixtures/example.rs:3:1"),
    Cow::from("Warning:  --> $DIR/example.rs:3:1")
)]
#[case::absolute(
    {
        let mut input = String::from(
            " --> /tmp/workspace/crates/rstest-bdd-macros/tests/fixtures/example.rs:4:2",
        );
        input.push(char::from(10));
        Cow::from(input)
    },
    {
        let mut expected = String::from(" --> $DIR/example.rs:4:2");
        expected.push(char::from(10));
        Cow::from(expected)
    }
)]
#[case::idempotent(
    Cow::from(" --> $DIR/example.rs:4:2"),
    Cow::from(" --> $DIR/example.rs:4:2")
)]
fn normalise_fixture_paths_cases(
    #[case] input: Cow<'static, str>,
    #[case] expected: Cow<'static, str>,
) {
    assert_eq!(normalise_fixture_paths(input.as_ref()), expected.as_ref());
}

#[test]
fn run_compile_fail_with_normalised_output_handles_multiple_normalisers() {
    const TEST_PATH: &str = "tests/fixtures/__normaliser_multiple.rs";
    let mut expected = String::from("error: missing step (hint-one)");
    expected.push(char::from(10));
    expected.push_str("help: review scenario (hint-two)");
    expected.push(char::from(10));
    let mut actual = String::from("error: missing step");
    actual.push(char::from(10));
    actual.push_str("help: review scenario");
    actual.push(char::from(10));
    let fixture = NormaliserFixture::new(
        FixturePath::new(TEST_PATH),
        expected.as_str(),
        actual.as_str(),
    );
    let strip_hint_one: Normaliser = |text| text.replace(" (hint-one)", "");
    let strip_hint_two: Normaliser = |text| text.replace(" (hint-two)", "");
    let result = panic::catch_unwind(|| {
        run_compile_fail_with_normalised_output(
            || panic!("expected failure"),
            FixturePath::new(TEST_PATH),
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
    const TEST_PATH: &str = "tests/fixtures/__normaliser_empty.rs";
    let fixture = NormaliserFixture::new(FixturePath::new(TEST_PATH), "", "");
    let result = panic::catch_unwind(|| {
        run_compile_fail_with_normalised_output(
            || panic!("expected failure"),
            FixturePath::new(TEST_PATH),
            &[],
        );
    });
    assert!(result.is_ok(), "identical empty outputs should be accepted");
    assert!(
        !fixture.actual_path.exists(),
        "matching outputs should delete the wip stderr file",
    );
}

#[rstest]
#[case(
    FixturePath::new("tests/fixtures/__normaliser_whitespace.rs"),
    "warning: trailing space",
    "warning: trailing space   ",
    true,
    "whitespace differences should be normalised",
    "matching outputs should delete the wip stderr file"
)]
#[case(
    FixturePath::new("tests/fixtures/__normaliser_unexpected.rs"),
    "error: expected formatting",
    "error: unexpected formatting",
    false,
    "mismatched outputs must propagate the panic",
    "mismatched outputs should retain the wip stderr file for inspection"
)]
fn run_compile_fail_with_normalised_output_test_cases(
    #[case] test_path: FixturePath<'static>,
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
    let fixture = NormaliserFixture::new(test_path, expected.as_str(), actual.as_str());
    let trim_trailing: Normaliser = |text| text.trim_end().to_owned();
    let result = panic::catch_unwind(|| {
        run_compile_fail_with_normalised_output(
            || panic!("expected failure"),
            test_path,
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
