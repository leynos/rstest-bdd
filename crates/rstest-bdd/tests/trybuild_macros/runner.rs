//! Process-wide environment orchestration for the trybuild integration run.
//!
//! This private module owns the single serialized test entrypoint. Fixture
//! paths and snapshot guards remain in `staging`; individual test groups stay
//! in the parent module so this boundary is not reused as general test setup.

use std::{env, io};

use camino::Utf8Path;

use super::{
    MacroFixtureCase,
    macros_fixture,
    run_conditional_ambiguous_step_test,
    run_conditional_ordering_tests,
    run_failing_macro_tests,
    run_failing_ui_tests,
    run_feature_tracking_test,
    run_lint_ui_tests,
    run_passing_macro_tests,
    staging,
    tracking,
};

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
    let crate_root = Utf8Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = crate_root
        .parent()
        .and_then(Utf8Path::parent)
        .expect("workspace root must be two levels above the manifest directory");
    let target_directory = staging::trybuild_target_directory(workspace_root);

    // Trybuild asks Cargo metadata for its target root. Cargo does not expose
    // an outer `--target-dir` flag to this test process, so explicitly pass
    // the running test's root to trybuild's nested Cargo invocations.
    temp_env::with_var("CARGO_TARGET_DIR", Some(target_directory.as_str()), || {
        temp_env::with_var_unset("RUST_BACKTRACE", || {
            let _target_root_snapshots = staging::stage_target_root_snapshots()?;
            let t = trybuild::TestCases::new();

            run_passing_macro_tests(&t);
            #[cfg(windows)]
            let alternate_root = staging::stage_unrelatable_feature_root()?;
            run_failing_macro_tests(&t);
            // POSIX absolute paths share `/`. Windows exercises D4 only when
            // the staged C: fixture differs from trybuild's target drive.
            #[cfg(windows)]
            if alternate_root.is_some() {
                t.compile_fail(
                    macros_fixture(MacroFixtureCase::from("scenario_unrelatable_path.rs"))
                        .as_std_path(),
                );
            }
            run_failing_ui_tests(&t)?;
            run_lint_ui_tests()?;
            t.compile_fail(
                macros_fixture(MacroFixtureCase::from("scenarios_missing_dir.rs")).as_std_path(),
            );
            run_conditional_ordering_tests(&t)?;
            run_conditional_ambiguous_step_test(&t);
            run_feature_tracking_test(&t);
            // `TestCases` runs its queued fixtures from `Drop`; inspect dep-info
            // only after the tracking fixture's dep-info is the final artefact.
            drop(t);
            tracking::assert_trybuild_tracking_registered_in_dep_info();
            Ok(())
        })
    })
}
