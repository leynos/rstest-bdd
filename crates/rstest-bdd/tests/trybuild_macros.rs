//! Compile-time tests for rstest-bdd procedural macros using trybuild.
//!
//! These tests verify that the `#[step]` and `#[scenario]` macros register
//! step definitions, surface compile-time validation errors, and emit clear
//! diagnostics. Trybuild executes the fixture crates and compares stderr
//! against checked-in snapshots.
//!
//! Normalisers rewrite fixture paths and strip nightly-only hints so the
//! assertions remain stable across platforms.

use std::borrow::Cow;
use std::fs;
use std::io;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use wrappers::{FixturePathLine, MacroFixtureCase, NormaliserInput, UiFixtureCase};

#[path = "trybuild_macros/wrappers.rs"]
mod wrappers;

const MACROS_FIXTURES_DIR: &str = "tests/fixtures_macros";
const UI_FIXTURES_DIR: &str = "tests/ui_macros";

fn macros_fixture(case: impl Into<MacroFixtureCase>) -> PathBuf {
    ensure_trybuild_support_files();
    let case = case.into();
    Path::new(MACROS_FIXTURES_DIR).join(case.as_ref())
}

fn ui_fixture(case: impl Into<UiFixtureCase>) -> PathBuf {
    let case = case.into();
    Path::new(UI_FIXTURES_DIR).join(case.as_ref())
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

    run_passing_macro_tests(&t);
    // `scenarios!` should succeed when the directory exists.
    // t.pass("tests/fixtures/scenarios_autodiscovery.rs");

    run_failing_macro_tests(&t);
    run_failing_ui_tests(&t);
    run_scenarios_missing_dir_test(&t);
    run_conditional_ordering_tests(&t);
    run_conditional_ambiguous_step_test(&t);
}

fn run_passing_macro_tests(t: &trybuild::TestCases) {
    for case in [
        MacroFixtureCase::from("step_macros.rs"),
        MacroFixtureCase::from("step_macros_unicode.rs"),
        MacroFixtureCase::from("scenario_single_match.rs"),
    ] {
        t.pass(macros_fixture(case));
    }
}

fn run_failing_macro_tests(t: &trybuild::TestCases) {
    for case in [
        MacroFixtureCase::from("scenario_missing_file.rs"),
        MacroFixtureCase::from("step_macros_invalid_identifier.rs"),
        MacroFixtureCase::from("step_tuple_pattern.rs"),
        MacroFixtureCase::from("step_struct_pattern.rs"),
        MacroFixtureCase::from("step_nested_pattern.rs"),
    ] {
        t.compile_fail(macros_fixture(case));
    }
}

fn run_failing_ui_tests(t: &trybuild::TestCases) {
    for case in [
        UiFixtureCase::from("datatable_wrong_type.rs"),
        UiFixtureCase::from("datatable_duplicate.rs"),
        UiFixtureCase::from("datatable_duplicate_attr.rs"),
        UiFixtureCase::from("datatable_after_docstring.rs"),
        UiFixtureCase::from("placeholder_missing_param.rs"),
        UiFixtureCase::from("implicit_fixture_missing.rs"),
        UiFixtureCase::from("placeholder_missing_params.rs"),
    ] {
        t.compile_fail(ui_fixture(case));
    }
}

fn run_scenarios_missing_dir_test(t: &trybuild::TestCases) {
    t.compile_fail(macros_fixture(MacroFixtureCase::from(
        "scenarios_missing_dir.rs",
    )));
}

#[expect(
    unexpected_cfgs,
    reason = "integration test inspects dependency feature flags"
)]
fn run_conditional_ordering_tests(t: &trybuild::TestCases) {
    let ordering_cases = [
        MacroFixtureCase::from("scenario_missing_step.rs"),
        MacroFixtureCase::from("scenario_out_of_order.rs"),
    ];

    if cfg!(feature = "strict-compile-time-validation") {
        for case in ordering_cases.iter().cloned() {
            t.compile_fail(macros_fixture(case));
        }
    } else {
        for case in ordering_cases.iter().cloned() {
            t.pass(macros_fixture(case));
        }
        compile_fail_missing_step_warning(t);
    }
}

#[expect(
    unexpected_cfgs,
    reason = "integration test inspects dependency feature flags"
)]
fn run_conditional_ambiguous_step_test(t: &trybuild::TestCases) {
    if cfg!(feature = "compile-time-validation") {
        t.compile_fail(macros_fixture(MacroFixtureCase::from(
            "scenario_ambiguous_step.rs",
        )));
    }
}

type Normaliser = for<'a> fn(NormaliserInput<'a>) -> String;

fn compile_fail_missing_step_warning(t: &trybuild::TestCases) {
    compile_fail_with_normalised_output(
        t,
        macros_fixture(MacroFixtureCase::from("scenario_missing_step_warning.rs")),
        &[strip_nightly_macro_backtrace_hint, normalise_fixture_paths],
    );
}

fn compile_fail_with_normalised_output(
    t: &trybuild::TestCases,
    test_path: impl AsRef<Path>,
    normalisers: &[Normaliser],
) {
    let test_path = test_path.as_ref();
    run_compile_fail_with_normalised_output(|| t.compile_fail(test_path), test_path, normalisers);
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

    if apply_normalisers(NormaliserInput::from(actual.as_ref()), normalisers)
        == apply_normalisers(NormaliserInput::from(expected.as_ref()), normalisers)
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
    let mut value = Cow::Borrowed(input.0);
    for normalise in normalisers {
        value = Cow::Owned(normalise(NormaliserInput::from(value.as_ref())));
    }
    value
}

fn normalise_fixture_paths(input: NormaliserInput<'_>) -> String {
    let text = input.as_ref();
    let normalised_lines = text
        .lines()
        .map(|line| normalise_fixture_path_line(FixturePathLine::from(line)))
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

    let value = line.as_ref();

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
    input.as_ref().replace(
        " (in Nightly builds, run with -Z macro-backtrace for more info)",
        "",
    )
}

#[cfg(test)]
#[path = "trybuild_macros/helper_tests.rs"]
mod helper_tests;
