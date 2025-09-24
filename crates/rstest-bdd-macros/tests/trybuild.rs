//! Compile-time tests for the procedural macros.

use std::borrow::Cow;
use std::fs;
use std::io;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};

#[test]
fn step_macros_compile() {
    let t = trybuild::TestCases::new();
    t.pass("tests/fixtures/step_macros.rs");
    t.pass("tests/fixtures/step_macros_unicode.rs");
    t.pass("tests/fixtures/scenario_single_match.rs");
    // `scenarios!` should succeed when the directory exists.
    // t.pass("tests/fixtures/scenarios_autodiscovery.rs");
    t.compile_fail("tests/fixtures/scenario_missing_file.rs");
    t.compile_fail("tests/fixtures/step_macros_invalid_identifier.rs");
    t.compile_fail("tests/fixtures/step_tuple_pattern.rs");
    t.compile_fail("tests/fixtures/step_struct_pattern.rs");
    t.compile_fail("tests/fixtures/step_nested_pattern.rs");
    t.compile_fail("tests/ui/datatable_wrong_type.rs");
    t.compile_fail("tests/ui/datatable_duplicate.rs");
    t.compile_fail("tests/ui/datatable_duplicate_attr.rs");
    t.compile_fail("tests/ui/datatable_after_docstring.rs");
    t.compile_fail("tests/ui/placeholder_missing_param.rs");
    t.compile_fail("tests/ui/implicit_fixture_missing.rs");
    t.compile_fail("tests/ui/placeholder_missing_params.rs");
    t.compile_fail("tests/fixtures/scenarios_missing_dir.rs");
    if cfg!(feature = "strict-compile-time-validation") {
        t.compile_fail("tests/fixtures/scenario_missing_step.rs");
        t.compile_fail("tests/fixtures/scenario_out_of_order.rs");
    } else {
        t.pass("tests/fixtures/scenario_missing_step.rs");
        t.pass("tests/fixtures/scenario_out_of_order.rs");
        compile_fail_missing_step_warning(&t);
    }
    if cfg!(feature = "compile-time-validation") {
        t.compile_fail("tests/fixtures/scenario_ambiguous_step.rs");
    }
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

fn compile_fail_missing_step_warning(t: &trybuild::TestCases) {
    compile_fail_with_normalised_output(
        t,
        FixturePath::new("tests/fixtures/scenario_missing_step_warning.rs"),
        &[strip_nightly_macro_backtrace_hint, normalise_fixture_paths],
    );
}

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

    #[test]
    fn expected_stderr_path_replaces_extension() {
        let path = expected_stderr_path(FixturePath::new("tests/ui/example.output"));
        assert_eq!(path, Path::new("tests/ui/example.stderr"));
    }

    #[test]
    fn expected_stderr_path_handles_multiple_extensions() {
        let path = expected_stderr_path(FixturePath::new("tests/ui/example.feature.rs"));
        assert_eq!(path, Path::new("tests/ui/example.feature.stderr"));
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

    #[test]
    fn apply_normalisers_handles_empty_string() {
        let trim_whitespace: Normaliser = |text| text.trim().to_owned();
        let result = apply_normalisers("", &[trim_whitespace]);
        assert_eq!(result, "");
    }

    #[test]
    fn apply_normalisers_handles_whitespace_only_string() {
        let trim_whitespace: Normaliser = |text| text.trim().to_owned();
        let mut whitespace = String::from("   ");
        whitespace.push(char::from(10));
        let result = apply_normalisers(whitespace.as_str(), &[trim_whitespace]);
        assert_eq!(result, "");
    }

    #[test]
    fn strip_nightly_macro_backtrace_hint_removes_multiple_instances() {
        let text = concat!(
            "error: failure",
            " (in Nightly builds, run with -Z macro-backtrace for more info)",
            " more context",
            " (in Nightly builds, run with -Z macro-backtrace for more info)"
        );
        let expected = "error: failure more context";
        assert_eq!(strip_nightly_macro_backtrace_hint(text), expected);
    }

    #[test]
    fn strip_nightly_macro_backtrace_hint_leaves_text_without_hint() {
        let text = "error: failure";
        assert_eq!(strip_nightly_macro_backtrace_hint(text), text);
    }

    #[test]
    fn normalise_fixture_paths_rewrites_relative_fixture_paths() {
        let dollar = char::from(36);
        let input = "Warning:  --> tests/fixtures/example.rs:3:1";
        let expected = format!("Warning:  --> {dollar}DIR/example.rs:3:1");
        assert_eq!(normalise_fixture_paths(input), expected);
    }

    #[test]
    fn normalise_fixture_paths_rewrites_absolute_fixture_paths() {
        let dollar = char::from(36);
        let newline = char::from(10);
        let input = format!(
            " --> /tmp/workspace/crates/rstest-bdd-macros/tests/fixtures/example.rs:4:2{newline}"
        );
        let expected = format!(" --> {dollar}DIR/example.rs:4:2{newline}");
        assert_eq!(normalise_fixture_paths(input.as_str()), expected);
    }

    #[test]
    fn normalise_fixture_paths_is_idempotent_for_normalised_input() {
        let dollar = char::from(36);
        let input = format!(" --> {dollar}DIR/example.rs:4:2");
        assert_eq!(normalise_fixture_paths(input.as_str()), input);
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
