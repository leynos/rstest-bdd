#![expect(
    unexpected_cfgs,
    reason = "integration test inspects dependency feature flags"
)]
//! Compile-time tests for the procedural macros.

use std::borrow::Cow;
use std::fs;
use std::io;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const MACROS_FIXTURES_DIR: &str = "tests/fixtures_macros";
const UI_FIXTURES_DIR: &str = "tests/ui_macros";

#[derive(Clone, Copy)]
struct MacroFixtureCase {
    file_name: &'static str,
}

#[derive(Clone, Copy)]
struct UiFixtureCase {
    file_name: &'static str,
}

#[derive(Clone, Copy)]
struct NormaliserInput<'a>(&'a str);

impl<'a> NormaliserInput<'a> {
    fn as_str(&self) -> &'a str {
        self.0
    }
}

#[derive(Clone, Copy)]
struct FixturePathLine<'a>(&'a str);

impl<'a> FixturePathLine<'a> {
    fn as_str(&self) -> &'a str {
        self.0
    }
}

fn macros_fixture(case: MacroFixtureCase) -> PathBuf {
    ensure_trybuild_support_files();
    Path::new(MACROS_FIXTURES_DIR).join(case.file_name)
}

fn ui_fixture(case: UiFixtureCase) -> PathBuf {
    Path::new(UI_FIXTURES_DIR).join(case.file_name)
}

fn ensure_trybuild_support_files() {
    static TRYBUILD_SUPPORT: OnceLock<()> = OnceLock::new();
    TRYBUILD_SUPPORT.get_or_init(|| {
        stage_trybuild_support_files().unwrap_or_else(|error| {
            panic!("failed to stage trybuild support files: {error}");
        });
    });
}

fn stage_trybuild_support_files() -> io::Result<()> {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = crate_root
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "workspace root must exist"))?;
    let target_tests_root = workspace_root.join("target/tests/trybuild");
    let trybuild_crate_root = target_tests_root.join("rstest-bdd");
    let workspace_features_root = target_tests_root.join("features");

    if workspace_features_root.exists() {
        fs::remove_dir_all(&workspace_features_root)?;
    }
    if trybuild_crate_root.exists() {
        fs::remove_dir_all(&trybuild_crate_root)?;
    }

    fs::create_dir_all(&workspace_features_root)?;
    fs::create_dir_all(&trybuild_crate_root)?;

    let features_root = crate_root.join("tests/features");
    let mut features = Vec::new();
    collect_feature_files(&features_root, &features_root, &mut features)?;
    features.sort_by(|a, b| a.0.cmp(&b.0));

    let fixtures_root = crate_root.join(MACROS_FIXTURES_DIR);
    let mut fixture_features = Vec::new();
    collect_feature_files(&fixtures_root, &fixtures_root, &mut fixture_features)?;
    fixture_features.sort_by(|a, b| a.0.cmp(&b.0));

    write_feature_files(&workspace_features_root, &features)?;
    write_feature_files(&trybuild_crate_root, &fixture_features)?;

    Ok(())
}

fn write_feature_files(destination_root: &Path, features: &[(String, String)]) -> io::Result<()> {
    for (relative, contents) in features {
        let mut path = destination_root.to_path_buf();
        for part in relative.split('/') {
            path.push(part);
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, contents)?;
    }

    Ok(())
}

fn collect_feature_files(
    root: &Path,
    current: &Path,
    features: &mut Vec<(String, String)>,
) -> io::Result<()> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_feature_files(root, &path, features)?;
            continue;
        }

        if path.extension().and_then(|ext| ext.to_str()) != Some("feature") {
            continue;
        }

        let relative = path.strip_prefix(root).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "feature path must be within the features directory",
            )
        })?;
        let relative = relative.to_string_lossy().replace(char::from(0x5C), "/");
        let contents = fs::read_to_string(&path)?;
        features.push((relative, contents));
    }

    Ok(())
}

#[test]
fn step_macros_compile() {
    let t = trybuild::TestCases::new();

    for case in [
        MacroFixtureCase {
            file_name: "step_macros.rs",
        },
        MacroFixtureCase {
            file_name: "step_macros_unicode.rs",
        },
        MacroFixtureCase {
            file_name: "scenario_single_match.rs",
        },
    ] {
        t.pass(macros_fixture(case));
    }
    // `scenarios!` should succeed when the directory exists.
    // t.pass("tests/fixtures/scenarios_autodiscovery.rs");

    for case in [
        MacroFixtureCase {
            file_name: "scenario_missing_file.rs",
        },
        MacroFixtureCase {
            file_name: "step_macros_invalid_identifier.rs",
        },
        MacroFixtureCase {
            file_name: "step_tuple_pattern.rs",
        },
        MacroFixtureCase {
            file_name: "step_struct_pattern.rs",
        },
        MacroFixtureCase {
            file_name: "step_nested_pattern.rs",
        },
    ] {
        t.compile_fail(macros_fixture(case));
    }

    for case in [
        UiFixtureCase {
            file_name: "datatable_wrong_type.rs",
        },
        UiFixtureCase {
            file_name: "datatable_duplicate.rs",
        },
        UiFixtureCase {
            file_name: "datatable_duplicate_attr.rs",
        },
        UiFixtureCase {
            file_name: "datatable_after_docstring.rs",
        },
        UiFixtureCase {
            file_name: "placeholder_missing_param.rs",
        },
        UiFixtureCase {
            file_name: "implicit_fixture_missing.rs",
        },
        UiFixtureCase {
            file_name: "placeholder_missing_params.rs",
        },
    ] {
        t.compile_fail(ui_fixture(case));
    }

    t.compile_fail(macros_fixture(MacroFixtureCase {
        file_name: "scenarios_missing_dir.rs",
    }));

    let ordering_cases = [
        MacroFixtureCase {
            file_name: "scenario_missing_step.rs",
        },
        MacroFixtureCase {
            file_name: "scenario_out_of_order.rs",
        },
    ];

    if cfg!(feature = "strict-compile-time-validation") {
        for case in ordering_cases {
            t.compile_fail(macros_fixture(case));
        }
    } else {
        for case in ordering_cases {
            t.pass(macros_fixture(case));
        }
        compile_fail_missing_step_warning(&t);
    }

    if cfg!(feature = "compile-time-validation") {
        t.compile_fail(macros_fixture(MacroFixtureCase {
            file_name: "scenario_ambiguous_step.rs",
        }));
    }
}

type Normaliser = for<'a> fn(NormaliserInput<'a>) -> String;

fn compile_fail_missing_step_warning(t: &trybuild::TestCases) {
    compile_fail_with_normalised_output(
        t,
        macros_fixture(MacroFixtureCase {
            file_name: "scenario_missing_step_warning.rs",
        }),
        &[strip_nightly_macro_backtrace_hint, normalise_fixture_paths],
    );
}

fn compile_fail_with_normalised_output(
    t: &trybuild::TestCases,
    test_path: impl AsRef<Path>,
    normalisers: &[Normaliser],
) {
    let test_path = test_path.as_ref();
    run_compile_fail_with_normalised_output(
        || t.compile_fail(test_path),
        Path::new(test_path),
        normalisers,
    );
}

fn run_compile_fail_with_normalised_output<F>(
    compile_fail: F,
    test_path: &Path,
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

fn normalised_outputs_match(test_path: &Path, normalisers: &[Normaliser]) -> io::Result<bool> {
    let actual_path = wip_stderr_path(test_path);
    let expected_path = expected_stderr_path(test_path);
    let actual = fs::read_to_string(&actual_path)?;
    let expected = fs::read_to_string(&expected_path)?;

    if apply_normalisers(NormaliserInput(actual.as_str()), normalisers)
        == apply_normalisers(NormaliserInput(expected.as_str()), normalisers)
    {
        let _ = fs::remove_file(actual_path);
        return Ok(true);
    }

    Ok(false)
}

fn wip_stderr_path(test_path: &Path) -> PathBuf {
    let Some(file_name) = test_path.file_name() else {
        panic!("trybuild test path must include file name");
    };
    let mut path = PathBuf::from(file_name);
    path.set_extension("stderr");
    Path::new("target/tests/wip").join(path)
}

fn expected_stderr_path(test_path: &Path) -> PathBuf {
    let mut path = PathBuf::from(test_path);
    path.set_extension("stderr");
    path
}

fn apply_normalisers<'a>(input: NormaliserInput<'a>, normalisers: &[Normaliser]) -> Cow<'a, str> {
    let mut value = Cow::Borrowed(input.as_str());
    for normalise in normalisers {
        value = Cow::Owned(normalise(NormaliserInput(value.as_ref())));
    }
    value
}

fn normalise_fixture_paths(input: NormaliserInput<'_>) -> String {
    let text = input.as_str();
    let normalised_lines = text
        .lines()
        .map(|line| normalise_fixture_path_line(FixturePathLine(line)))
        .collect::<Vec<_>>();
    let separator = char::from(0x0A);
    let separator_str = separator.to_string();
    let mut normalised = normalised_lines.join(&separator_str);
    if text.ends_with(separator) {
        normalised.push(separator);
    }
    normalised
}

fn normalise_fixture_path_line(line: FixturePathLine<'_>) -> String {
    const ARROW: &str = "-->";

    let value = line.as_str();

    let Some((prefix, remainder)) = value.split_once(ARROW) else {
        return value.to_owned();
    };

    let trimmed = remainder.trim_start();
    if trimmed.is_empty() || !trimmed.contains(".rs") {
        return value.to_owned();
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

fn strip_nightly_macro_backtrace_hint(input: NormaliserInput<'_>) -> String {
    input.as_str().replace(
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

    #[derive(Clone, Copy)]
    struct FixtureTestPath<'a>(&'a str);

    impl<'a> FixtureTestPath<'a> {
        fn as_str(&self) -> &'a str {
            self.0
        }
    }

    #[derive(Clone, Copy)]
    struct FixtureStderr<'a>(&'a str);

    impl<'a> FixtureStderr<'a> {
        fn as_str(&self) -> &'a str {
            self.0
        }
    }

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
            let test_path = Path::new(test_path.as_str());

            let expected_path = expected_stderr_path(test_path);
            if let Some(parent) = expected_path.parent() {
                fs::create_dir_all(parent).unwrap_or_else(|error| {
                    panic!("failed to create directory for expected stderr fixture: {error}");
                });
            }
            fs::write(&expected_path, expected.as_str()).unwrap_or_else(|error| {
                panic!("failed to write expected stderr fixture: {error}");
            });

            let actual_path = wip_stderr_path(test_path);
            if let Some(parent) = actual_path.parent() {
                fs::create_dir_all(parent).unwrap_or_else(|error| {
                    panic!("failed to create directory for wip stderr fixture: {error}");
                });
            }
            fs::write(&actual_path, actual.as_str()).unwrap_or_else(|error| {
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
        let result = apply_normalisers(NormaliserInput("message"), &[]);
        assert!(matches!(result, Cow::Borrowed("message")));
    }

    #[test]
    fn apply_normalisers_respects_normaliser_order() {
        let add_prefix: Normaliser = |input| format!("prefix-{}", input.as_str());
        let add_suffix: Normaliser = |input| format!("{}-suffix", input.as_str());
        let result = apply_normalisers(NormaliserInput("value"), &[add_prefix, add_suffix]);
        assert_eq!(result, "prefix-value-suffix");
    }

    #[test]
    fn apply_normalisers_handles_empty_string() {
        let trim_whitespace: Normaliser = |input| input.as_str().trim().to_owned();
        let result = apply_normalisers(NormaliserInput(""), &[trim_whitespace]);
        assert_eq!(result, "");
    }

    #[test]
    fn apply_normalisers_handles_whitespace_only_string() {
        let trim_whitespace: Normaliser = |input| input.as_str().trim().to_owned();
        let mut whitespace = String::from("   ");
        whitespace.push(char::from(10));
        let result = apply_normalisers(NormaliserInput(whitespace.as_str()), &[trim_whitespace]);
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
        assert_eq!(
            strip_nightly_macro_backtrace_hint(NormaliserInput(text)),
            expected
        );
    }

    #[test]
    fn strip_nightly_macro_backtrace_hint_leaves_text_without_hint() {
        let text = "error: failure";
        assert_eq!(
            strip_nightly_macro_backtrace_hint(NormaliserInput(text)),
            text
        );
    }

    #[test]
    fn normalise_fixture_paths_rewrites_relative_fixture_paths() {
        let dollar = char::from(36);
        let input = "Warning:  --> tests/fixtures_macros/example.rs:3:1";
        let expected = format!("Warning:  --> {dollar}DIR/example.rs:3:1");
        assert_eq!(normalise_fixture_paths(NormaliserInput(input)), expected);
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
            normalise_fixture_paths(NormaliserInput(input.as_str())),
            expected
        );
    }

    #[test]
    fn normalise_fixture_paths_is_idempotent_for_normalised_input() {
        let dollar = char::from(36);
        let input = format!(" --> {dollar}DIR/example.rs:4:2");
        assert_eq!(
            normalise_fixture_paths(NormaliserInput(input.as_str())),
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
            FixtureStderr(expected.as_str()),
            FixtureStderr(actual.as_str()),
        );
        let strip_hint_one: Normaliser = |input| input.as_str().replace(" (hint-one)", "");
        let strip_hint_two: Normaliser = |input| input.as_str().replace(" (hint-two)", "");
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
        "tests/fixtures_macros/__normaliser_unexpected.rs",
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
            FixtureStderr(expected.as_str()),
            FixtureStderr(actual.as_str()),
        );
        let trim_trailing: Normaliser = |input| input.as_str().trim_end().to_owned();
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
}
