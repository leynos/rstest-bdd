//! Compile-time tests for rstest-bdd procedural macros using trybuild.
//! These tests verify that the `#[step]` and `#[scenario]` macros register
//! step definitions, surface compile-time validation errors, and emit clear
//! diagnostics. Trybuild executes the fixture crates and compares stderr
//! against checked-in snapshots.
//!
//! Normalizers rewrite fixture paths and strip nightly-only hints so the
//! assertions remain stable across platforms.
use std::{
    borrow::Cow,
    env,
    io,
    panic::{self, AssertUnwindSafe},
    path::Path as StdPath,
    process::Command,
};

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs::Dir};
use wrappers::{
    MacroFixtureCase,
    NormalizerInput,
    UiFixtureCase,
    normalize_conditional_trait_help,
    normalize_fixture_paths,
    strip_nightly_macro_backtrace_hint,
};

#[path = "trybuild_macros/staging.rs"]
mod staging;

#[path = "trybuild_macros/tracking.rs"]
mod tracking;
#[path = "trybuild_macros/whitaker.rs"]
mod whitaker;
#[path = "trybuild_macros/wip.rs"]
mod wip;
#[path = "trybuild_macros/wrappers.rs"]
mod wrappers;

use wip::{read_wip_stderr, remove_stale_wip_stderr, wip_stderr_path};

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
#[serial_test::serial(trybuild_target_directory)]
fn step_macros_compile() -> io::Result<()> {
    // This test spawns `cargo` (through `trybuild` and `cargo clippy`). On
    // Windows, nextest wraps test binaries in Job Objects and those child
    // processes inherit the write end of nextest's capture pipe, which never
    // closes; see "nextest on Windows: trybuild deadlock" in
    // `docs/developers-guide.md`. Skip only on that platform combination so
    // the fixtures still run under nextest everywhere else. The
    // `rstest-bdd::trybuild_macros` test group in `.config/nextest.toml`
    // keeps this binary from running alongside other cargo-spawning tests.
    if cfg!(windows) && env::var_os("NEXTEST_RUN_ID").is_some() {
        return Ok(());
    }
    // Prevent RUST_BACKTRACE from contaminating compiler diagnostic output.
    // Trybuild snapshots are compared verbatim; CI's `RUST_BACKTRACE=short`
    // injects backtrace fragments on Windows MSVC that diverge from the
    // Linux-generated snapshots. Trybuild tests are about structured
    // diagnostics, not runtime backtraces.
    //
    // Rust 2024 makes `std::env::remove_var` unsafe; Rust 1.85.0 is the first
    // release supporting that edition. This workspace forbids unsafe code, so
    // `temp_env` provides the same scoped mutation without weakening that lint.
    temp_env::with_var_unset("RUST_BACKTRACE", || {
        let _target_root_snapshots = staging::stage_target_root_snapshots()?;
        let t = trybuild::TestCases::new();

        run_passing_macro_tests(&t);
        #[cfg(windows)]
        let _alternate_root = staging::stage_unrelatable_feature_root()?;
        run_failing_macro_tests(&t);
        run_failing_ui_tests(&t)?;
        run_lint_ui_tests()?;
        t.compile_fail(
            macros_fixture(MacroFixtureCase::from("scenarios_missing_dir.rs")).as_std_path(),
        );
        run_conditional_ordering_tests(&t)?;
        run_conditional_ambiguous_step_test(&t);
        // `TestCases` runs its queued fixtures from `Drop`; inspect dep-info
        // only after this run has actually compiled the tracking fixture.
        drop(t);
        tracking::assert_trybuild_tracking_registered_in_dep_info();
        Ok(())
    })
}
fn run_passing_macro_tests(t: &trybuild::TestCases) {
    for case in [
        MacroFixtureCase::from("step_macros.rs"),
        MacroFixtureCase::from("step_macros_unicode.rs"),
        MacroFixtureCase::from("scenario_single_match.rs"),
        MacroFixtureCase::from("scenario_feature_tracking.rs"),
        MacroFixtureCase::from("scenario_state_default.rs"),
        MacroFixtureCase::from("scenarios_fixtures.rs"),
        MacroFixtureCase::from("scenarios_autodiscovery.rs"),
        MacroFixtureCase::from("scenario_harness_params.rs"),
        MacroFixtureCase::from("scenario_third_party_harness_cookbook.rs"),
        MacroFixtureCase::from("scenario_bulk_migration_cookbook.rs"),
        MacroFixtureCase::from("scenario_harness_failing.rs"),
        MacroFixtureCase::from("scenario_async_step_tokio_bridge.rs"),
        MacroFixtureCase::from("execution_policy_reexports.rs"),
        MacroFixtureCase::from("scenarios_harness_params.rs"),
        MacroFixtureCase::from("harness_context_coexist.rs"),
        MacroFixtureCase::from("step_fixture_requirements/all_immutable_fixtures.rs"),
        MacroFixtureCase::from("step_fixture_requirements/fixture_requirements_emitted.rs"),
        MacroFixtureCase::from("step_fixture_requirements/single_mutable_fixture.rs"),
        MacroFixtureCase::from("step_fixture_requirements/mixed_mutability_fixtures.rs"),
        MacroFixtureCase::from("step_fixture_requirements/two_mutable_fixtures.rs"),
        MacroFixtureCase::from("step_return_dispatch_lint_clean.rs"),
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
        MacroFixtureCase::from("scenario_outline_harness_async_rejected.rs"),
        MacroFixtureCase::from("result_fixture_requires_result_scenario.rs"),
    ] {
        t.compile_fail(macros_fixture(case).as_std_path());
    }

    // D4's unrelatable-root diagnostic (different Windows drive or UNC
    // prefix): on POSIX every absolute path shares `/`, so the fixture can
    // only fail where the case is real. The Windows CI legs exercise and pin
    // it via `scenario_unrelatable_path.stderr`.
    #[cfg(windows)]
    t.compile_fail(
        macros_fixture(MacroFixtureCase::from("scenario_unrelatable_path.rs")).as_std_path(),
    );
}

fn run_failing_ui_tests(t: &trybuild::TestCases) -> io::Result<()> {
    for case in [
        UiFixtureCase::from("datatable_wrong_type.rs"),
        UiFixtureCase::from("datatable_duplicate.rs"),
        UiFixtureCase::from("datatable_duplicate_attr.rs"),
        UiFixtureCase::from("datatable_conflicting_map.rs"),
        UiFixtureCase::from("datatable_optional_requires_option.rs"),
        UiFixtureCase::from("datatable_optional_with_default.rs"),
        UiFixtureCase::from("datatable_truthy_with_parse_with.rs"),
        UiFixtureCase::from("datatable_after_docstring.rs"),
        UiFixtureCase::from("from_duplicate.rs"),
        UiFixtureCase::from("harness_context_with_from.rs"),
        UiFixtureCase::from("harness_context_with_datatable.rs"),
        UiFixtureCase::from("harness_context_with_step_args.rs"),
        UiFixtureCase::from("harness_context_takes_no_arguments.rs"),
        UiFixtureCase::from("harness_context_duplicate.rs"),
        UiFixtureCase::from("harness_context_on_placeholder.rs"),
        UiFixtureCase::from("placeholder_missing_param.rs"),
        UiFixtureCase::from("implicit_fixture_missing.rs"),
        UiFixtureCase::from("placeholder_missing_params.rs"),
        UiFixtureCase::from("return_override_result_requires_result.rs"),
        UiFixtureCase::from("step_return_nested_result.rs"),
        UiFixtureCase::from("step_return_impl_trait.rs"),
        UiFixtureCase::from("insert_value_must_use.rs"),
    ] {
        t.compile_fail(ui_fixture(case).as_std_path());
    }
    compile_fail_with_normalized_output(
        ui_fixture(UiFixtureCase::from(
            "step_return_alias_error_not_display.rs",
        )),
        &[normalize_conditional_trait_help],
    )
}

fn run_lint_ui_tests() -> io::Result<()> {
    let cases = [
        (
            "scenario_unused_fixture_param",
            &["-D", "unused_variables"][..],
        ),
        (
            "scenario_underscore_fixture_param",
            &["-D", "clippy::used_underscore_binding"][..],
        ),
        ("step_return_dispatch_lint_clean", &["-D", "warnings"][..]),
    ];

    for (bin, lint_args) in cases {
        run_lint_ui_case(bin, lint_args)?;
    }

    Ok(())
}

fn run_lint_ui_case(bin: &str, lint_args: &[&str]) -> io::Result<()> {
    let manifest_dir = Utf8Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest_path = manifest_dir.join("tests/ui_lints/Cargo.toml");
    // Keep lint runs isolated from the workspace target directory to avoid
    // artefact/cache conflicts and cross-test contamination; this makes
    // per-bin `cargo clippy` checks deterministic.
    let target_dir = manifest_dir.join("target/tests/ui_lints_clippy");
    let output = Command::new("cargo")
        .current_dir(manifest_dir.as_std_path())
        .env("CARGO_TARGET_DIR", target_dir.as_str())
        .arg("clippy")
        .arg("--locked")
        .arg("--manifest-path")
        .arg(manifest_path.as_str())
        .arg("--bin")
        .arg(bin)
        .arg("--")
        .args(lint_args)
        .output()?;

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!(
            "cargo clippy failed for {bin}\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
            output.status, stdout, stderr
        );
    }

    Ok(())
}

#[expect(
    unexpected_cfgs,
    reason = "integration test inspects dependency feature flags"
)]
fn run_conditional_ordering_tests(t: &trybuild::TestCases) -> io::Result<()> {
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
        compile_fail_missing_step_warning(t)?;
    }

    Ok(())
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

type Normalizer = for<'a> fn(NormalizerInput<'a>) -> String;

#[rustversion::not(nightly)]
fn compile_fail_missing_step_warning(_t: &trybuild::TestCases) -> io::Result<()> {
    compile_fail_with_normalized_output(
        macros_fixture(MacroFixtureCase::from("scenario_missing_step_warning.rs")),
        &[strip_nightly_macro_backtrace_hint, normalize_fixture_paths],
    )
}

#[rustversion::nightly]
fn compile_fail_missing_step_warning(_t: &trybuild::TestCases) -> io::Result<()> {
    compile_fail_with_normalized_output(
        macros_fixture(MacroFixtureCase::from("nightly_registry_warning.rs")),
        &[strip_nightly_macro_backtrace_hint, normalize_fixture_paths],
    )
}

fn compile_fail_with_normalized_output(
    test_path: impl AsRef<Utf8Path>,
    normalizers: &[Normalizer],
) -> io::Result<()> {
    let test_path = test_path.as_ref();
    let crate_dir = Dir::open_ambient_dir(
        Utf8Path::new(env!("CARGO_MANIFEST_DIR")).as_std_path(),
        ambient_authority(),
    )?;
    let expected_path = expected_stderr_path(test_path.as_std_path());
    let expected = crate_dir.read_to_string(expected_path.as_std_path())?;

    remove_stale_wip_stderr(test_path)?;
    crate_dir.remove_file(expected_path.as_std_path())?;

    run_compile_fail_with_normalized_output(
        || {
            let t = trybuild::TestCases::new();
            t.compile_fail(test_path.as_std_path());
        },
        || crate_dir.write(expected_path.as_std_path(), expected.as_bytes()),
        test_path,
        normalizers,
    )
}

fn run_compile_fail_with_normalized_output<F, R>(
    compile_fail: F,
    restore_expected: R,
    test_path: &Utf8Path,
    normalizers: &[Normalizer],
) -> io::Result<()>
where
    F: FnOnce(),
    R: FnOnce() -> io::Result<()>,
{
    let compilation = panic::catch_unwind(AssertUnwindSafe(compile_fail));
    restore_expected()?;

    match compilation {
        Ok(()) => Ok(()),
        Err(panic) => {
            if normalized_outputs_match(test_path, normalizers)? {
                return Ok(());
            }

            panic::resume_unwind(panic);
        }
    }
}

fn normalized_outputs_match(test_path: &Utf8Path, normalizers: &[Normalizer]) -> io::Result<bool> {
    let crate_dir = Dir::open_ambient_dir(
        Utf8Path::new(env!("CARGO_MANIFEST_DIR")).as_std_path(),
        ambient_authority(),
    )?;
    let expected_path = expected_stderr_path(test_path.as_std_path());
    let current_dir = Dir::open_ambient_dir(".", ambient_authority())?;
    let actual_path = wip_stderr_path(test_path.as_std_path());
    let (actual, is_in_current_dir) =
        read_wip_stderr(&current_dir, &crate_dir, actual_path.as_std_path())?;
    let expected = crate_dir.read_to_string(expected_path.as_std_path())?;

    if apply_normalizers(NormalizerInput::from(actual.as_str()), normalizers)
        == apply_normalizers(NormalizerInput::from(expected.as_str()), normalizers)
    {
        let wip_dir = if is_in_current_dir {
            &current_dir
        } else {
            &crate_dir
        };
        wip_dir.remove_file(actual_path.as_std_path())?;
        return Ok(true);
    }

    Ok(false)
}

fn expected_stderr_path(test_path: &StdPath) -> Utf8PathBuf {
    let Ok(mut path) = Utf8PathBuf::from_path_buf(test_path.to_path_buf()) else {
        panic!("test_path must be valid UTF-8");
    };
    path.set_extension("stderr");
    path
}

fn apply_normalizers<'a>(input: NormalizerInput<'a>, normalizers: &[Normalizer]) -> Cow<'a, str> {
    let mut value = Cow::Borrowed(input.0);
    for normalize in normalizers {
        value = Cow::Owned(normalize(NormalizerInput::from(value.as_ref())));
    }
    value
}
#[cfg(test)]
#[path = "trybuild_macros/helper_tests.rs"]
mod helper_tests;
