//! Basic smoke tests for the cargo-bdd subcommand.

use assert_cmd::Command;
use eyre::{Context, Result};
use serde_json::Value;
use serial_test::serial;
use std::fs;
use std::path::PathBuf;
use std::str;

/// Execute cargo-bdd with the given arguments and return the raw output.
fn run_cargo_bdd_raw(args: &[&str]) -> Result<std::process::Output> {
    let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/minimal");
    let target_dir = fixture_dir.join("target");
    fs::create_dir_all(&target_dir)
        .with_context(|| format!("failed to create {}", target_dir.display()))?;
    let mut cmd = Command::cargo_bin("cargo-bdd")
        .wrap_err("cargo-bdd binary should exist in this workspace")?;
    cmd.current_dir(fixture_dir)
        .env("CARGO_TARGET_DIR", &target_dir)
        .args(args)
        .output()
        .wrap_err("failed to execute `cargo bdd`")
}

fn run_cargo_bdd(args: &[&str]) -> Result<String> {
    let output = run_cargo_bdd_raw(args)?;
    assert!(output.status.success(), "`cargo bdd` should succeed");
    let stdout = str::from_utf8(&output.stdout).wrap_err("`cargo bdd` emitted invalid UTF-8")?;
    Ok(stdout.to_string())
}

fn run_cargo_bdd_failure(args: &[&str]) -> Result<String> {
    let output = run_cargo_bdd_raw(args)?;
    assert!(
        !output.status.success(),
        "`cargo bdd` should fail for invalid arguments"
    );
    let stderr =
        str::from_utf8(&output.stderr).wrap_err("`cargo bdd` emitted invalid UTF-8 to stderr")?;
    Ok(stderr.to_string())
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
fn skipped_subcommand_emits_json() -> Result<()> {
    let stdout = run_cargo_bdd(&["skipped", "--json"])?;
    let parsed: Value = serde_json::from_str(&stdout)?;
    let entries = parsed
        .as_array()
        .ok_or_else(|| eyre::eyre!("skipped output should be a JSON array"))?;
    let fixture_entry = entries
        .iter()
        .find(|entry| {
            entry.get("scenario") == Some(&Value::String("fixture skipped scenario".to_string()))
        })
        .ok_or_else(|| eyre::eyre!("expected fixture skipped scenario entry"))?;
    assert_eq!(
        fixture_entry.get("feature").and_then(Value::as_str),
        Some("tests/features/diagnostics.fixture"),
    );
    assert_eq!(
        fixture_entry.get("reason").and_then(Value::as_str),
        Some("fixture skip message"),
    );
    assert!(fixture_entry.get("line").and_then(Value::as_u64).is_some());
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
fn steps_skipped_emits_json() -> Result<()> {
    let stdout = run_cargo_bdd(&["steps", "--skipped", "--json"])?;
    let parsed: Value = serde_json::from_str(&stdout)?;
    let entries = parsed
        .as_array()
        .ok_or_else(|| eyre::eyre!("steps --skipped output should be an array"))?;
    let forced_entry = entries
        .iter()
        .find(|entry| {
            entry.get("scenario") == Some(&Value::String("fixture forced failure skip".to_string()))
        })
        .ok_or_else(|| eyre::eyre!("expected forced failure skip entry"))?;
    assert_eq!(
        forced_entry.get("reason").and_then(Value::as_str),
        Some("fixture forced skip"),
    );
    assert!(
        forced_entry.get("step").is_some(),
        "bypassed steps should include step info"
    );
    Ok(())
}

#[test]
fn steps_json_requires_skipped_flag() -> Result<()> {
    let stderr = run_cargo_bdd_failure(&["steps", "--json"])?;
    assert!(
        stderr.contains("--json") && stderr.contains("--skipped"),
        "error should mention --json requires --skipped: {stderr}"
    );
    Ok(())
}
