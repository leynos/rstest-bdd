//! Compile-time tests for rstest-bdd procedural macros using trybuild.
//!
//! These tests verify that the `#[step]` and `#[scenario]` macros register
//! step definitions, surface compile-time validation errors, and emit clear
//! diagnostics. Trybuild executes the fixture crates and compares stderr
//! against checked-in snapshots.
//!
//! Normalisers rewrite fixture paths and strip nightly-only hints so the
//! assertions remain stable across platforms.

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs::Dir};
use std::borrow::Cow;
use std::io;
use std::panic::{self, AssertUnwindSafe};
use std::path::Path as StdPath;
use std::sync::OnceLock;
use wrappers::{FixturePathLine, MacroFixtureCase, NormaliserInput, UiFixtureCase};

#[path = "trybuild_macros/wrappers.rs"]
mod wrappers;

const MACROS_FIXTURES_DIR: &str = "tests/fixtures_macros";
const UI_FIXTURES_DIR: &str = "tests/ui_macros";

fn macros_fixture(case: impl Into<MacroFixtureCase>) -> Utf8PathBuf {
    ensure_trybuild_support_files();
    let case = case.into();
    let case_str: &str = case.as_ref();
    Utf8PathBuf::from(MACROS_FIXTURES_DIR).join(case_str)
}

fn ui_fixture(case: impl Into<UiFixtureCase>) -> Utf8PathBuf {
    let case = case.into();
    let case_str: &str = case.as_ref();
    Utf8PathBuf::from(UI_FIXTURES_DIR).join(case_str)
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
    let crate_root = Utf8Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = crate_root
        .parent()
        .and_then(Utf8Path::parent)
        .map(Utf8Path::to_owned)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "workspace root must exist"))?;
    let workspace_dir = Dir::open_ambient_dir(workspace_root.as_std_path(), ambient_authority())?;

    let target_tests_relative = Utf8Path::new("target/tests/trybuild");
    let trybuild_crate_relative = target_tests_relative.join("rstest-bdd");
    let workspace_features_relative = target_tests_relative.join("features");

    remove_dir_if_exists(&workspace_dir, workspace_features_relative.as_path())?;
    remove_dir_if_exists(&workspace_dir, trybuild_crate_relative.as_path())?;

    workspace_dir.create_dir_all(workspace_features_relative.as_std_path())?;
    workspace_dir.create_dir_all(trybuild_crate_relative.as_std_path())?;

    let crate_dir = Dir::open_ambient_dir(crate_root.as_std_path(), ambient_authority())?;
    let features_dir = crate_dir.open_dir("tests/features")?;
    let mut features = Vec::new();
    collect_feature_files(&features_dir, Utf8Path::new("."), &mut features)?;
    features.sort_by(|a, b| a.0.cmp(&b.0));

    let fixtures_dir = crate_dir.open_dir(MACROS_FIXTURES_DIR)?;
    let mut fixture_features = Vec::new();
    collect_feature_files(&fixtures_dir, Utf8Path::new("."), &mut fixture_features)?;
    fixture_features.sort_by(|a, b| a.0.cmp(&b.0));

    write_feature_files(
        &workspace_dir,
        workspace_features_relative.as_std_path(),
        &features,
    )?;
    write_feature_files(
        &workspace_dir,
        trybuild_crate_relative.as_std_path(),
        &fixture_features,
    )?;

    Ok(())
}

fn write_feature_files(
    root: &Dir,
    destination_root: &StdPath,
    features: &[(String, String)],
) -> io::Result<()> {
    let destination_root =
        Utf8PathBuf::from_path_buf(destination_root.to_path_buf()).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "destination_root must be valid UTF-8",
            )
        })?;

    for (relative, contents) in features {
        let path = destination_root.join(relative);
        if let Some(parent) = path.parent() {
            root.create_dir_all(parent.as_std_path())?;
        }
        root.write(path.as_std_path(), contents.as_bytes())?;
    }

    Ok(())
}

fn remove_dir_if_exists(root: &Dir, path: &Utf8Path) -> io::Result<()> {
    match root.remove_dir_all(path.as_std_path()) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn collect_feature_files(
    dir: &Dir,
    current: &Utf8Path,
    features: &mut Vec<(String, String)>,
) -> io::Result<()> {
    let is_root = current == Utf8Path::new(".");
    for entry in dir.read_dir(current.as_std_path())? {
        let entry = entry?;
        let file_name = entry.file_name().to_string_lossy().into_owned();
        let relative = if is_root {
            Utf8PathBuf::from(file_name.as_str())
        } else {
            current.join(file_name.as_str())
        };

        if entry.file_type()?.is_dir() {
            collect_feature_files(dir, relative.as_path(), features)?;
            continue;
        }

        if !file_name.ends_with(".feature") {
            continue;
        }

        let contents = dir.read_to_string(relative.as_std_path())?;
        features.push((relative.to_string(), contents));
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
        t.pass(macros_fixture(case).as_std_path());
    }
}

fn run_failing_macro_tests(t: &trybuild::TestCases) {
    for case in [
        MacroFixtureCase::from("scenario_missing_file.rs"),
        MacroFixtureCase::from("scenario_missing_name.rs"),
        MacroFixtureCase::from("scenario_missing_name_empty.rs"),
        MacroFixtureCase::from("scenario_name_and_index.rs"),
        MacroFixtureCase::from("scenario_duplicate_name.rs"),
        MacroFixtureCase::from("step_macros_invalid_identifier.rs"),
        MacroFixtureCase::from("step_tuple_pattern.rs"),
        MacroFixtureCase::from("step_struct_pattern.rs"),
        MacroFixtureCase::from("step_nested_pattern.rs"),
    ] {
        t.compile_fail(macros_fixture(case).as_std_path());
    }
}

fn run_failing_ui_tests(t: &trybuild::TestCases) {
    for case in [
        UiFixtureCase::from("datatable_wrong_type.rs"),
        UiFixtureCase::from("datatable_duplicate.rs"),
        UiFixtureCase::from("datatable_duplicate_attr.rs"),
        UiFixtureCase::from("datatable_conflicting_map.rs"),
        UiFixtureCase::from("datatable_optional_requires_option.rs"),
        UiFixtureCase::from("datatable_optional_with_default.rs"),
        UiFixtureCase::from("datatable_truthy_with_parse_with.rs"),
        UiFixtureCase::from("datatable_after_docstring.rs"),
        UiFixtureCase::from("placeholder_missing_param.rs"),
        UiFixtureCase::from("implicit_fixture_missing.rs"),
        UiFixtureCase::from("placeholder_missing_params.rs"),
    ] {
        t.compile_fail(ui_fixture(case).as_std_path());
    }
}

fn run_scenarios_missing_dir_test(t: &trybuild::TestCases) {
    t.compile_fail(
        macros_fixture(MacroFixtureCase::from("scenarios_missing_dir.rs")).as_std_path(),
    );
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
            t.compile_fail(macros_fixture(case).as_std_path());
        }
    } else {
        for case in ordering_cases.iter().cloned() {
            t.pass(macros_fixture(case).as_std_path());
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
        t.compile_fail(
            macros_fixture(MacroFixtureCase::from("scenario_ambiguous_step.rs")).as_std_path(),
        );
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
    test_path: impl AsRef<Utf8Path>,
    normalisers: &[Normaliser],
) {
    let test_path = test_path.as_ref();
    run_compile_fail_with_normalised_output(
        || t.compile_fail(test_path.as_std_path()),
        test_path,
        normalisers,
    );
}

fn run_compile_fail_with_normalised_output<F>(
    compile_fail: F,
    test_path: &Utf8Path,
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

fn normalised_outputs_match(test_path: &Utf8Path, normalisers: &[Normaliser]) -> io::Result<bool> {
    let crate_dir = Dir::open_ambient_dir(
        Utf8Path::new(env!("CARGO_MANIFEST_DIR")).as_std_path(),
        ambient_authority(),
    )?;
    let actual_path = wip_stderr_path(test_path.as_std_path());
    let expected_path = expected_stderr_path(test_path.as_std_path());
    let actual = crate_dir.read_to_string(actual_path.as_std_path())?;
    let expected = crate_dir.read_to_string(expected_path.as_std_path())?;

    if apply_normalisers(NormaliserInput::from(actual.as_str()), normalisers)
        == apply_normalisers(NormaliserInput::from(expected.as_str()), normalisers)
    {
        let _ = crate_dir.remove_file(actual_path.as_std_path());
        return Ok(true);
    }

    Ok(false)
}

fn wip_stderr_path(test_path: &StdPath) -> Utf8PathBuf {
    let Some(file_name) = test_path.file_name() else {
        panic!("trybuild test path must include file name");
    };
    let file_name = file_name
        .to_str()
        .unwrap_or_else(|| panic!("file name must be valid UTF-8"));
    let mut path = Utf8PathBuf::from(file_name);
    path.set_extension("stderr");
    Utf8PathBuf::from("target/tests/wip").join(path)
}

fn expected_stderr_path(test_path: &StdPath) -> Utf8PathBuf {
    let mut path = Utf8PathBuf::from_path_buf(test_path.to_path_buf())
        .unwrap_or_else(|_| panic!("test_path must be valid UTF-8"));
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
    let mut normalised = text
        .lines()
        .map(|line| normalise_fixture_path_line(FixturePathLine::from(line)))
        .collect::<Vec<_>>()
        .join("\n");
    if text.ends_with('\n') {
        normalised.push('\n');
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

    let file_name = Utf8Path::new(path).file_name().unwrap_or(path);

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
