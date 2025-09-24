//! Compile-time tests for the procedural macros.

use rstest::rstest;
use std::borrow::Cow;
use std::fs;
use std::io;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};

#[rstest]
#[case::step_macros(FixturePath::new("tests/fixtures/step_macros.rs"))]
#[case::step_macros_unicode(FixturePath::new("tests/fixtures/step_macros_unicode.rs"))]
#[case::scenario_single_match(FixturePath::new("tests/fixtures/scenario_single_match.rs"))]
#[cfg_attr(
    not(feature = "strict-compile-time-validation"),
    case::scenario_missing_step(FixturePath::new("tests/fixtures/scenario_missing_step.rs"))
)]
#[cfg_attr(
    not(feature = "strict-compile-time-validation"),
    case::scenario_out_of_order(FixturePath::new("tests/fixtures/scenario_out_of_order.rs"))
)]
fn trybuild_fixtures_pass(#[case] fixture: FixturePath<'static>) {
    let t = trybuild::TestCases::new();
    t.pass(fixture.as_str());
}

#[rstest]
#[case::scenario_missing_file(FixturePath::new("tests/fixtures/scenario_missing_file.rs"))]
#[case::step_macros_invalid_identifier(FixturePath::new(
    "tests/fixtures/step_macros_invalid_identifier.rs"
))]
#[case::step_tuple_pattern(FixturePath::new("tests/fixtures/step_tuple_pattern.rs"))]
#[case::step_struct_pattern(FixturePath::new("tests/fixtures/step_struct_pattern.rs"))]
#[case::step_nested_pattern(FixturePath::new("tests/fixtures/step_nested_pattern.rs"))]
#[case::datatable_wrong_type(FixturePath::new("tests/ui/datatable_wrong_type.rs"))]
#[case::datatable_duplicate(FixturePath::new("tests/ui/datatable_duplicate.rs"))]
#[case::datatable_duplicate_attr(FixturePath::new("tests/ui/datatable_duplicate_attr.rs"))]
#[case::datatable_after_docstring(FixturePath::new("tests/ui/datatable_after_docstring.rs"))]
#[case::placeholder_missing_param(FixturePath::new("tests/ui/placeholder_missing_param.rs"))]
#[case::implicit_fixture_missing(FixturePath::new("tests/ui/implicit_fixture_missing.rs"))]
#[case::placeholder_missing_params(FixturePath::new("tests/ui/placeholder_missing_params.rs"))]
#[case::scenarios_missing_dir(FixturePath::new("tests/fixtures/scenarios_missing_dir.rs"))]
#[cfg_attr(
    feature = "strict-compile-time-validation",
    case::scenario_missing_step(FixturePath::new("tests/fixtures/scenario_missing_step.rs"))
)]
#[cfg_attr(
    feature = "strict-compile-time-validation",
    case::scenario_out_of_order(FixturePath::new("tests/fixtures/scenario_out_of_order.rs"))
)]
#[cfg_attr(
    feature = "compile-time-validation",
    case::scenario_ambiguous_step(FixturePath::new("tests/fixtures/scenario_ambiguous_step.rs"))
)]
fn trybuild_fixtures_compile_fail(#[case] fixture: FixturePath<'static>) {
    let t = trybuild::TestCases::new();
    t.compile_fail(fixture.as_str());
}

#[cfg(not(feature = "strict-compile-time-validation"))]
#[test]
fn scenario_missing_step_emits_warning() {
    let t = trybuild::TestCases::new();
    compile_fail_missing_step_warning(&t);
}

type Normaliser = fn(&str) -> String;

#[derive(Clone, Copy)]
struct FixturePath<'a> {
    raw: &'a str,
}

impl<'a> FixturePath<'a> {
    fn new(raw: &'a str) -> Self {
        Self { raw }
    }

    fn as_str(self) -> &'a str {
        self.raw
    }

    fn expected_stderr_path(self) -> PathBuf {
        let mut path = PathBuf::from(self.raw);
        path.set_extension("stderr");
        path
    }

    fn wip_stderr_path(self) -> PathBuf {
        let Some(file_name) = Path::new(self.raw).file_name() else {
            panic!("trybuild test path must include file name");
        };
        let mut path = PathBuf::from(file_name);
        path.set_extension("stderr");
        Path::new("target/tests/wip").join(path)
    }
}

impl<'a> From<&'a str> for FixturePath<'a> {
    fn from(raw: &'a str) -> Self {
        FixturePath::new(raw)
    }
}

#[cfg(not(feature = "strict-compile-time-validation"))]
fn compile_fail_missing_step_warning(t: &trybuild::TestCases) {
    compile_fail_with_normalised_output(
        t,
        FixturePath::new("tests/fixtures/scenario_missing_step_warning.rs"),
        &[strip_nightly_macro_backtrace_hint, normalise_fixture_paths],
    );
}

#[cfg(not(feature = "strict-compile-time-validation"))]
fn compile_fail_with_normalised_output(
    t: &trybuild::TestCases,
    test_path: FixturePath<'_>,
    normalisers: &[Normaliser],
) {
    run_compile_fail_with_normalised_output(
        || t.compile_fail(test_path.as_str()),
        test_path,
        normalisers,
    );
}

fn run_compile_fail_with_normalised_output<F>(
    compile_fail: F,
    test_path: FixturePath<'_>,
    normalisers: &[Normaliser],
) where
    F: FnOnce(),
{
    match panic::catch_unwind(AssertUnwindSafe(compile_fail)) {
        Ok(()) => (),
        Err(panic) => {
            if normalised_outputs_match(test_path, normalisers).unwrap_or(false) {
                return;
            }

            panic::resume_unwind(panic);
        }
    }
}

fn normalised_outputs_match(
    test_path: FixturePath<'_>,
    normalisers: &[Normaliser],
) -> io::Result<bool> {
    let actual_path = wip_stderr_path(test_path);
    let expected_path = expected_stderr_path(test_path);
    let actual = fs::read_to_string(&actual_path)?;
    let expected = fs::read_to_string(&expected_path)?;

    if apply_normalisers(&actual, normalisers) == apply_normalisers(&expected, normalisers) {
        let _ = fs::remove_file(actual_path);
        return Ok(true);
    }

    Ok(false)
}

fn wip_stderr_path(test_path: FixturePath<'_>) -> PathBuf {
    test_path.wip_stderr_path()
}

fn expected_stderr_path(test_path: FixturePath<'_>) -> PathBuf {
    test_path.expected_stderr_path()
}

fn apply_normalisers<'a>(text: &'a str, normalisers: &[Normaliser]) -> Cow<'a, str> {
    let mut value = Cow::Borrowed(text);
    for normalise in normalisers {
        value = Cow::Owned(normalise(value.as_ref()));
    }
    value
}

fn normalise_fixture_paths(text: &str) -> String {
    let normalised_lines = text
        .lines()
        .map(normalise_fixture_path_line)
        .collect::<Vec<_>>();
    let separator = char::from(0x0A);
    let separator_str = separator.to_string();
    let mut normalised = normalised_lines.join(&separator_str);
    if text.ends_with(separator) {
        normalised.push(separator);
    }
    normalised
}

fn normalise_fixture_path_line(line: &str) -> String {
    const ARROW: &str = "-->";

    let Some((prefix, remainder)) = line.split_once(ARROW) else {
        return line.to_owned();
    };

    let trimmed = remainder.trim_start();
    if trimmed.is_empty() || !trimmed.contains(".rs") {
        return line.to_owned();
    }

    let mut parts = trimmed.splitn(2, ':');
    let path = parts.next().unwrap_or(trimmed);
    let suffix = parts.next();

    let file_name = Path::new(path)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(path);

    let mut rebuilt = format!("{prefix}{ARROW} ");
    rebuilt.push('$');
    rebuilt.push_str("DIR/");
    rebuilt.push_str(file_name);
    if let Some(rest) = suffix {
        if !rest.is_empty() {
            rebuilt.push(':');
            rebuilt.push_str(rest);
        }
    }

    rebuilt
}

fn strip_nightly_macro_backtrace_hint(text: &str) -> String {
    text.replace(
        " (in Nightly builds, run with -Z macro-backtrace for more info)",
        "",
    )
}

#[cfg(test)]
mod helper_tests {
    use super::*;
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
        fn new(test_path: FixturePath<'_>, expected: &str, actual: &str) -> Self {
            let expected_path = expected_stderr_path(test_path);
            if let Some(parent) = expected_path.parent() {
                fs::create_dir_all(parent).unwrap_or_else(|error| {
                    panic!("failed to create directory for expected stderr fixture: {error}");
                });
            }
            fs::write(&expected_path, expected).unwrap_or_else(|error| {
                panic!("failed to write expected stderr fixture: {error}");
            });

            let actual_path = wip_stderr_path(test_path);
            if let Some(parent) = actual_path.parent() {
                fs::create_dir_all(parent).unwrap_or_else(|error| {
                    panic!("failed to create directory for wip stderr fixture: {error}");
                });
            }
            fs::write(&actual_path, actual).unwrap_or_else(|error| {
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
        let path = wip_stderr_path(FixturePath::new("tests/fixtures/__helper_case.rs"));
        assert_eq!(path, Path::new("target/tests/wip/__helper_case.stderr"));
    }

    #[test]
    #[should_panic(expected = "trybuild test path must include file name")]
    fn wip_stderr_path_panics_without_file_name() {
        wip_stderr_path(FixturePath::new(""));
    }

    #[rstest]
    #[case::single_extension(
        FixturePath::new("tests/ui/example.output"),
        "tests/ui/example.stderr"
    )]
    #[case::multiple_extensions(
        FixturePath::new("tests/ui/example.feature.rs"),
        "tests/ui/example.feature.stderr"
    )]
    fn expected_stderr_path_rewrites_extension(
        #[case] input: FixturePath<'static>,
        #[case] expected: &str,
    ) {
        let path = expected_stderr_path(input);
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
}
