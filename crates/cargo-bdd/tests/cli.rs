//! Basic smoke tests for the cargo-bdd subcommand.

use assert_cmd::cargo::cargo_bin_cmd;
use eyre::{Context, Result};
use serde::Deserialize;
use serial_test::serial;
use std::fs;
use std::path::PathBuf;
use std::process::ExitStatus;
use std::str;

#[derive(Debug, Deserialize)]
struct SkippedDefinition {
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
    reason: Option<String>,
    #[serde(default)]
    step: Option<SkippedDefinition>,
}

/// Execute cargo-bdd with the given arguments and return the raw output.
fn run_cargo_bdd_raw(args: &[&str]) -> Result<std::process::Output> {
    let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/minimal");
    // Reuse the workspace target directory so path dependencies (rstest-bdd and
    // rstest-bdd-macros) are already compiled before invoking `cargo bdd`.
    //
    // This avoids slow first-run compiles within the fixture directory causing
    // nextest's per-test slow-timeout to terminate CLI smoke tests.
    let target_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target");
    fs::create_dir_all(&target_dir)
        .with_context(|| format!("failed to create {}", target_dir.display()))?;
    let mut cmd = cargo_bin_cmd!("cargo-bdd");
    cmd.current_dir(fixture_dir)
        .env("CARGO_TARGET_DIR", &target_dir)
        .args(args)
        .output()
        .wrap_err("failed to execute `cargo bdd`")
}

fn run_cargo_bdd_captured(args: &[&str]) -> Result<(ExitStatus, String, String)> {
    let output = run_cargo_bdd_raw(args)?;
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
    Ok((status, stdout.to_string(), stderr.to_string()))
}

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

fn run_cargo_bdd_steps() -> Result<String> {
    run_cargo_bdd(&["steps"])
}

#[test]
#[serial]
fn list_steps_runs() -> Result<()> {
    let stdout = run_cargo_bdd_steps()?;
    assert!(
        !stdout.is_empty(),
        "Expected non-empty output from steps command",
    );
    Ok(())
}

#[test]
#[serial]
fn steps_output_includes_skipped_statuses() -> Result<()> {
    let stdout = run_cargo_bdd_steps()?;
    assert!(
        stdout.contains("skipped tests/features/diagnostics.fixture :: fixture skipped scenario"),
        "expected skipped scenario heading in cargo bdd output: {stdout}"
    );
    assert!(
        stdout.contains("fixture skip message"),
        "expected skip message to appear in cargo bdd output: {stdout}"
    );
    Ok(())
}

#[test]
#[serial]
fn steps_output_marks_forced_failure_skips() -> Result<()> {
    let stdout = run_cargo_bdd_steps()?;
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
    Ok(())
}

#[test]
#[serial]
fn skipped_subcommand_includes_reasons_and_lines() -> Result<()> {
    let stdout = run_cargo_bdd(&["skipped", "--reasons"])?;
    assert!(
        stdout.contains("tests/features/diagnostics.fixture:7"),
        "skipped output should include feature location",
    );
    assert!(
        stdout.contains("fixture skip message"),
        "skip reason should appear in skipped output",
    );
    assert!(stdout.contains("[forced failure]"));
    Ok(())
}

#[test]
#[serial]
fn skipped_subcommand_emits_json() -> Result<()> {
    let stdout = run_cargo_bdd(&["skipped", "--json"])?;
    let entries: Vec<SkipReport> = serde_json::from_str(&stdout)?;
    let fixture_entry = entries
        .iter()
        .find(|entry| entry.scenario == "fixture skipped scenario")
        .ok_or_else(|| eyre::eyre!("expected fixture skipped scenario entry"))?;
    assert_eq!(fixture_entry.feature, "tests/features/diagnostics.fixture",);
    assert_eq!(
        fixture_entry.reason.as_deref(),
        Some("fixture skip message"),
    );
    assert!(fixture_entry.line > 0, "expected a 1-based line number");
    assert_eq!(
        fixture_entry.tags,
        vec!["@allow_skipped".to_string()],
        "expected fixture skipped scenario tags to be preserved"
    );
    Ok(())
}

#[test]
#[serial]
fn steps_skipped_outputs_bypassed_definitions() -> Result<()> {
    let stdout = run_cargo_bdd(&["steps", "--skipped"])?;
    assert!(stdout.contains("fixture bypassed step"));
    assert!(stdout.contains("fixture skip message"));
    Ok(())
}

#[test]
#[serial]
fn steps_skipped_emits_json() -> Result<()> {
    let stdout = run_cargo_bdd(&["steps", "--skipped", "--json"])?;
    let entries: Vec<SkipReport> = serde_json::from_str(&stdout)?;
    let forced_entry = entries
        .iter()
        .find(|entry| entry.scenario == "fixture forced failure skip")
        .ok_or_else(|| eyre::eyre!("expected forced failure skip entry"))?;
    assert_eq!(forced_entry.reason.as_deref(), Some("fixture forced skip"),);
    assert!(
        forced_entry.step.is_some(),
        "bypassed steps should include step info"
    );
    let Some(step) = forced_entry.step.as_ref() else {
        unreachable!("assertion above ensures step is present");
    };
    assert_eq!(step.keyword, "Then");
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
    Ok(())
}

#[test]
#[serial]
fn steps_json_requires_skipped_flag() -> Result<()> {
    let stderr = run_cargo_bdd_failure(&["steps", "--json"])?;
    assert!(
        stderr.contains("--json") && stderr.contains("--skipped"),
        "error should mention --json requires --skipped: {stderr}"
    );
    Ok(())
}
