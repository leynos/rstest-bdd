//! Smoke tests for the `cargo-bdd` subcommand, driven as a real subprocess.
//!
//! Each test that runs the binary locates or builds it with
//! `locate_or_build_binary`, wraps the result in an `assert_cmd::Command` and
//! drives it against the fixture crate in `tests/fixtures/minimal`, asserting
//! on the captured stdout, stderr and exit status. The suite covers `steps`
//! with and without `--skipped`, and `skipped` with `--reasons` and with
//! `--json`, including the skipped-scenario and forced-failure reporting
//! paths, the handling of test binaries that cannot dump steps, and the
//! rejection of `steps --json` without `--skipped`.
//!
//! Every child is spawned through `nested_cargo::filtered_command` with the
//! map the private `cli_child_environment` helper builds. That helper
//! supplies the workspace `target/` as the fallback and preserves a
//! caller-provided `CARGO_TARGET_DIR` (the coverage target directory when
//! `cargo llvm-cov` is running), so the nested builds reuse the workspace's
//! compiled units instead of rebuilding the fixture's own graph. The child
//! keeps its inherited `LLVM_PROFILE_FILE`, because `cargo-bdd` is the
//! process under test: redirecting that profile would report the lines it
//! exercised as never executed.
//!
//! The nested-build target policy has behavioural coverage of its own in the
//! `nested_target` submodule, attached with `#[path = "nested_target/mod.rs"]`
//! so Cargo does not also compile it as a second test target.
//!
//! The tests that drive the binary each spawn a nested Cargo build against
//! the shared workspace `target/`, so they carry `#[serial]`: under
//! `cargo test` the annotation holds them one at a time in the shared test
//! process, and `.config/nextest.toml` puts the binary in a `cargo-spawning`
//! group with `max-threads = 1` under nextest. Either way they do not contend
//! for the same target directory and the package-cache lock. The environment
//! map's precedence is asserted separately by
//! `cli_child_environment_reuses_target`.

use std::{env, ffi::OsString, path::PathBuf, process::ExitStatus, str};

use assert_cmd::Command;
use eyre::{Context, Result};
use rstest_bdd_harness::{
    binary_test_support::{BinaryName, locate_or_build_binary},
    nested_cargo,
};
use serde::Deserialize;
use serial_test::serial;

#[derive(Debug, Deserialize)]
struct SkippedDefinition {
    library: String,
    keyword: String,
    pattern: String,
    file: String,
    line: u32,
}

#[derive(Debug, Deserialize)]
struct SkipReport {
    feature: String,
    scenario: String,
    line: u32,
    tags: Vec<String>,
    #[serde(default)]
    libraries: Vec<String>,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    step: Option<SkippedDefinition>,
}

/// The fixture crate the CLI smoke tests build and inspect.
fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/minimal")
}

/// Execute cargo-bdd with extra environment variables and return the output.
///
/// Configuration tests use this to exercise the `CARGO_BDD_` environment
/// tier without changing the ambient environment of the test process.
fn run_cargo_bdd_raw_with_env(
    args: &[&str],
    extra_environment: &[(&str, &str)],
) -> Result<std::process::Output> {
    let fixture_dir = fixture_dir();
    // Preserve the caller's target directory (cargo llvm-cov's
    // `target/llvm-cov-target` when coverage is running) so the fixture build
    // stays warm; the filtered environment stops `cargo-bdd` and its nested
    // `cargo` children from inheriting stale jobserver or coverage control
    // variables that hang or corrupt a coverage run.
    //
    // `cargo-bdd` is the process under test, so it keeps the inherited
    // `LLVM_PROFILE_FILE`: `cargo llvm-cov` only merges the `.profraw` files
    // named by the pattern it exported, and redirecting this one would report
    // its exercised lines as never executed.
    let child_env = cli_child_environment(env::vars_os().collect())?;
    let program = locate_or_build_cargo_bdd_command()?;
    let mut cmd = nested_cargo::filtered_command(
        std::path::Path::new(program.get_program()),
        &child_env,
        &fixture_dir,
    );
    cmd.args(args);
    for (key, value) in extra_environment {
        cmd.env(key, value);
    }
    Ok(cmd.output()?)
}

/// Locate the workspace-built binary, building it when absent.
fn locate_or_build_cargo_bdd_command() -> Result<Command> {
    let root = workspace_root();
    locate_or_build_binary(
        &root.join("Cargo.toml"),
        &root,
        BinaryName::new("cargo-bdd"),
    )
    .map(Command::from_std)
    .map_err(|e| eyre::eyre!(e))
}

/// Repository root the CLI fixture and nested builds live under.
fn workspace_root() -> PathBuf { PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..") }

// The nested-build target policy has its own behavioural coverage; it lives in
// a subdirectory so Cargo does not also discover it as a second test target.
#[path = "nested_target/mod.rs"]
mod nested_target;

// Preparation of the fixture's nested build, kept out of this module so the
// timed tests and their setup stay separately readable.
#[path = "cli/preparation.rs"]
mod preparation;

use preparation::ensure_cli_prepared;

// The file-tier configuration case lives beside this target for the same
// reason: Cargo must not discover it as a second test target.
#[path = "cli/configuration.rs"]
mod configuration;

/// Apply the shared isolation policy with this CLI fixture's workspace target.
///
/// Only CLI smoke-test setup owns this fallback; explicit coverage and caller
/// target directories continue to take precedence.
///
/// For example, `cli_child_environment(Vec::new())` selects the workspace
/// `target/`; a snapshot containing `CARGO_TARGET_DIR` retains that value.
///
/// # Errors
///
/// As [`nested_cargo::env_from_vars`]: a relative target directory that cannot
/// be resolved against this process's working directory is reported rather than
/// handed to a child that would resolve it somewhere else.
fn cli_child_environment(
    vars: Vec<(OsString, OsString)>,
) -> std::io::Result<nested_cargo::NestedCargoEnv> {
    nested_cargo::env_from_vars(
        vars,
        Some(&workspace_root().join("target")),
        nested_cargo::ProfileDestination::Caller,
    )
}

/// The child keeps a caller target directory and otherwise uses the workspace.
#[rstest::rstest]
#[case::workspace_cache(None)]
#[case::caller_cache(Some("custom-coverage-target"))]
fn cli_child_environment_reuses_target(#[case] target: Option<&str>) -> std::io::Result<()> {
    let vars = target
        .map(|value| vec![(OsString::from("CARGO_TARGET_DIR"), OsString::from(value))])
        .unwrap_or_default();
    let child = cli_child_environment(vars)?;
    // A caller's value is preserved as a *directory*, not necessarily as the
    // exact string: a relative value names a different place to a child that
    // runs in the fixture directory, so the helper resolves it against this
    // process's working directory before handing it over. The workspace
    // fallback is already absolute.
    let expected = target.map_or_else(
        || workspace_root().join("target"),
        |value| std::env::current_dir().expect("current dir").join(value),
    );
    assert_eq!(child.get("CARGO_TARGET_DIR"), Some(expected.as_os_str()));
    assert_eq!(
        child
            .target_directory()
            .expect("a resolved environment should yield its target directory")
            .as_deref(),
        Some(expected.as_path()),
        "the resolved target directory must be the absolute one the child receives"
    );
    Ok(())
}

/// Execute `cargo bdd` and capture its status and streams as text.
fn run_cargo_bdd_captured(args: &[&str]) -> Result<(ExitStatus, String, String)> {
    run_cargo_bdd_captured_with_env(args, &[])
}

/// Execute cargo-bdd with extra environment variables and capture its streams.
fn run_cargo_bdd_captured_with_env(
    args: &[&str],
    extra_environment: &[(&str, &str)],
) -> Result<(ExitStatus, String, String)> {
    let output = run_cargo_bdd_raw_with_env(args, extra_environment)?;
    let args_debug = args.join(" ");
    let status = output.status;
    let stdout = str::from_utf8(&output.stdout).wrap_err_with(|| {
        format!(
            "`cargo bdd` emitted invalid UTF-8 to stdout (args: [{args_debug}], status: {status})"
        )
    })?;
    let stderr = str::from_utf8(&output.stderr).wrap_err_with(|| {
        format!(
            "`cargo bdd` emitted invalid UTF-8 to stderr (args: [{args_debug}], status: {status})"
        )
    })?;
    Ok((status, stdout.to_owned(), stderr.to_owned()))
}

/// Execute `cargo bdd` with no extra environment and return stdout.
fn run_cargo_bdd(args: &[&str]) -> Result<String> {
    let args_debug = args.join(" ");
    let (status, stdout, _stderr) = run_cargo_bdd_captured(args)?;
    assert!(
        status.success(),
        "`cargo bdd` should succeed (args: [{args_debug}], status: {status})"
    );
    Ok(stdout)
}

fn run_cargo_bdd_failure(args: &[&str]) -> Result<String> {
    let args_debug = args.join(" ");
    let (status, _stdout, stderr) = run_cargo_bdd_captured(args)?;
    assert!(
        !status.success(),
        "`cargo bdd` should fail for invalid arguments (args: [{args_debug}], status: {status})"
    );
    Ok(stderr)
}

fn run_cargo_bdd_steps() -> Result<String> { run_cargo_bdd(&["steps"]) }

#[test]
#[serial]
fn list_steps_runs() {
    ensure_cli_prepared().expect("test setup should succeed");
    let stdout = run_cargo_bdd_steps().expect("test setup should succeed");
    assert!(
        !stdout.is_empty(),
        "Expected non-empty output from steps command",
    );
}

#[test]
#[serial]
fn steps_skips_test_binaries_without_dump_steps_support() {
    ensure_cli_prepared().expect("test setup should succeed");
    let stdout = run_cargo_bdd_steps().expect("test setup should succeed");
    assert!(
        stdout.contains("fixture bypassed step"),
        "supported test targets should still be reported: {stdout}"
    );
    assert!(
        !stdout.contains("ordinary test target has no registry dump"),
        "unsupported test targets must not contribute fallback registry output: {stdout}"
    );
}

#[test]
#[serial]
fn steps_output_includes_skipped_statuses() {
    ensure_cli_prepared().expect("test setup should succeed");
    let stdout = run_cargo_bdd_steps().expect("test setup should succeed");
    assert!(
        stdout.contains("skipped tests/features/diagnostics.fixture :: fixture skipped scenario"),
        "expected skipped scenario heading in cargo bdd output: {stdout}"
    );
    assert!(
        stdout.contains("fixture skip message"),
        "expected skip message to appear in cargo bdd output: {stdout}"
    );
}

#[test]
#[serial]
fn steps_output_marks_forced_failure_skips() {
    ensure_cli_prepared().expect("test setup should succeed");
    let stdout = run_cargo_bdd_steps().expect("test setup should succeed");
    assert!(
        stdout.contains("[forced failure]"),
        "expected forced failure annotation in cargo bdd output: {stdout}",
    );
    assert!(
        stdout.contains("fixture forced failure skip"),
        "expected forced failure skip scenario heading in cargo bdd output: {stdout}",
    );
    assert!(
        stdout.contains("fixture forced skip"),
        "expected forced skip message in cargo bdd output: {stdout}",
    );
}

/// Text output includes the feature location, reason and annotations.
#[test]
#[serial]
fn skipped_subcommand_includes_reasons_and_lines() {
    ensure_cli_prepared().expect("test setup should succeed");
    let stdout = run_cargo_bdd(&["skipped", "--reasons"]).expect("test setup should succeed");
    assert!(
        stdout.contains("tests/features/diagnostics.fixture:7"),
        "skipped output should include feature location",
    );
    assert!(
        stdout.contains("fixture skip message"),
        "skip reason should appear in skipped output",
    );
    assert!(stdout.contains("[forced failure]"));
}

/// The JSON listing describes the fixture's skipped scenario.
#[test]
#[serial]
fn skipped_subcommand_emits_json() {
    ensure_cli_prepared().expect("test setup should succeed");
    let stdout = run_cargo_bdd(&["skipped", "--json"]).expect("test setup should succeed");
    let entries: Vec<SkipReport> =
        serde_json::from_str(&stdout).expect("test setup should succeed");
    let fixture_entry = entries
        .iter()
        .find(|entry| entry.scenario == "fixture skipped scenario")
        .ok_or_else(|| eyre::eyre!("expected fixture skipped scenario entry"))
        .expect("test setup should succeed");
    assert_eq!(fixture_entry.feature, "tests/features/diagnostics.fixture",);
    assert_eq!(
        fixture_entry.reason.as_deref(),
        Some("fixture skip message"),
    );
    assert!(fixture_entry.line > 0, "expected a 1-based line number");
    assert_eq!(
        fixture_entry.tags,
        vec!["@allow_skipped".to_owned()],
        "expected fixture skipped scenario tags to be preserved"
    );
    assert_eq!(
        fixture_entry.libraries,
        vec!["rstest_bdd::global".to_owned()],
        "unscoped scenarios should report the global library"
    );
}

#[test]
#[serial]
fn steps_skipped_outputs_bypassed_definitions() {
    ensure_cli_prepared().expect("test setup should succeed");
    let stdout = run_cargo_bdd(&["steps", "--skipped"]).expect("test setup should succeed");
    assert!(stdout.contains("fixture bypassed step"));
    assert!(stdout.contains("fixture skip message"));
}

#[test]
#[serial]
fn steps_skipped_emits_json() {
    ensure_cli_prepared().expect("test setup should succeed");
    let stdout =
        run_cargo_bdd(&["steps", "--skipped", "--json"]).expect("test setup should succeed");
    let entries: Vec<SkipReport> =
        serde_json::from_str(&stdout).expect("test setup should succeed");
    let forced_entry = entries
        .iter()
        .find(|entry| entry.scenario == "fixture forced failure skip")
        .ok_or_else(|| eyre::eyre!("expected forced failure skip entry"))
        .expect("test setup should succeed");
    assert_eq!(forced_entry.reason.as_deref(), Some("fixture forced skip"),);
    assert!(
        forced_entry.step.is_some(),
        "bypassed steps should include step info"
    );
    let Some(step) = forced_entry.step.as_ref() else {
        unreachable!("assertion above ensures step is present");
    };
    assert_eq!(step.keyword, "Then");
    assert_eq!(step.library, "rstest_bdd::global");
    assert_eq!(step.pattern, "fixture forced bypass");
    assert!(
        step.file.contains("diagnostics_fixture.rs"),
        "expected step file path to include diagnostics_fixture.rs, got: {}",
        step.file
    );
    assert!(
        step.line > 0,
        "expected bypassed step to include a line number"
    );
    assert_eq!(
        forced_entry.libraries,
        vec!["rstest_bdd::global".to_owned()],
        "bypassed steps should retain the scenario library scope"
    );
}

#[test]
#[serial]
fn steps_json_requires_skipped_flag() {
    let stderr = run_cargo_bdd_failure(&["steps", "--json"]).expect("test setup should succeed");
    assert!(
        stderr.contains("--json") && stderr.contains("--skipped"),
        "error should mention --json requires --skipped: {stderr}"
    );
}
