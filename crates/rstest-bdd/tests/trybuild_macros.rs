//! Compile-time tests for rstest-bdd procedural macros using trybuild.
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
use std::env;
use std::io;
use std::panic::{self, AssertUnwindSafe};
use std::path::Path as StdPath;
use wrappers::{
    MacroFixtureCase, NormaliserInput, UiFixtureCase, normalise_fixture_paths,
    strip_nightly_macro_backtrace_hint,
};

#[path = "trybuild_macros/staging.rs"]
mod staging;
#[path = "trybuild_macros/wrappers.rs"]
mod wrappers;

fn macros_fixture(case: impl Into<MacroFixtureCase>) -> Utf8PathBuf {
    let case = case.into();
    let case_str: &str = case.as_ref();
    staging::macros_fixture(case_str)
}

fn ui_fixture(case: impl Into<UiFixtureCase>) -> Utf8PathBuf {
    let case = case.into();
    let case_str: &str = case.as_ref();
    staging::ui_fixture(case_str)
}

#[test]
fn step_macros_compile() {
    if env::var_os("NEXTEST_RUN_ID").is_some() {
        return;
    }
    let t = trybuild::TestCases::new();

    run_passing_macro_tests(&t);
    run_failing_macro_tests(&t);
    run_failing_ui_tests(&t);
    t.compile_fail(
        macros_fixture(MacroFixtureCase::from("scenarios_missing_dir.rs")).as_std_path(),
    );
    run_conditional_ordering_tests(&t);
    run_conditional_ambiguous_step_test(&t);
}

fn run_passing_macro_tests(t: &trybuild::TestCases) {
    for case in [
        MacroFixtureCase::from("step_macros.rs"),
        MacroFixtureCase::from("step_macros_unicode.rs"),
        MacroFixtureCase::from("scenario_single_match.rs"),
        MacroFixtureCase::from("scenario_state_default.rs"),
        MacroFixtureCase::from("scenarios_fixtures.rs"),
        MacroFixtureCase::from("scenarios_autodiscovery.rs"),
        MacroFixtureCase::from("scenario_harness_params.rs"),
        MacroFixtureCase::from("scenario_attributes_tokio.rs"),
        MacroFixtureCase::from("scenario_attributes_tokio_sync.rs"),
        MacroFixtureCase::from("scenario_attributes_tokio_dedup.rs"),
        MacroFixtureCase::from("scenarios_harness_params.rs"),
    ] {
        t.pass(macros_fixture(case).as_std_path());
    }
}

fn run_failing_macro_tests(t: &trybuild::TestCases) {
    for case in [
        MacroFixtureCase::from("scenario_missing_file.rs"),
        MacroFixtureCase::from("scenario_missing_name.rs"),
        MacroFixtureCase::from("scenario_missing_name_empty.rs"),
        MacroFixtureCase::from("scenario_missing_path.rs"),
        MacroFixtureCase::from("scenario_result_requires_unit.rs"),
        MacroFixtureCase::from("scenario_step_result_requires_unit.rs"),
        MacroFixtureCase::from("scenario_name_and_index.rs"),
        MacroFixtureCase::from("scenario_index_out_of_range.rs"),
        MacroFixtureCase::from("scenario_duplicate_name.rs"),
        MacroFixtureCase::from("scenario_tags_no_match.rs"),
        MacroFixtureCase::from("step_macros_invalid_identifier.rs"),
        MacroFixtureCase::from("step_tuple_pattern.rs"),
        MacroFixtureCase::from("step_struct_pattern.rs"),
        MacroFixtureCase::from("step_nested_pattern.rs"),
        MacroFixtureCase::from("scenarios_fixtures_duplicate.rs"),
        MacroFixtureCase::from("scenarios_fixtures_malformed.rs"),
        MacroFixtureCase::from("scenarios_autodiscovery_invalid_path.rs"),
        MacroFixtureCase::from("outline_undefined_placeholder.rs"),
        MacroFixtureCase::from("scenario_harness_invalid.rs"),
        MacroFixtureCase::from("scenario_attributes_invalid.rs"),
        MacroFixtureCase::from("scenarios_harness_invalid.rs"),
        MacroFixtureCase::from("scenarios_attributes_invalid.rs"),
        MacroFixtureCase::from("scenario_harness_not_default.rs"),
        MacroFixtureCase::from("scenario_harness_async_rejected.rs"),
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
        UiFixtureCase::from("return_override_result_requires_result.rs"),
    ] {
        t.compile_fail(ui_fixture(case).as_std_path());
    }
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
#[cfg(test)]
#[path = "trybuild_macros/helper_tests.rs"]
mod helper_tests;
