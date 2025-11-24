//! Basic smoke tests for the cargo-bdd subcommand.

use assert_cmd::Command;
use eyre::{Context, Result};
use serial_test::serial;
use std::fs;
use std::path::PathBuf;
use std::str;

fn run_cargo_bdd_steps() -> Result<String> {
    let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/minimal");
    let target_dir = fixture_dir.join("target");
    fs::create_dir_all(&target_dir)
        .with_context(|| format!("failed to create {}", target_dir.display()))?;
    let mut cmd = Command::cargo_bin("cargo-bdd")
        .wrap_err("cargo-bdd binary should exist in this workspace")?;
    let output = cmd
        .current_dir(fixture_dir)
        .env("CARGO_TARGET_DIR", &target_dir)
        .arg("steps")
        .output()
        .wrap_err("failed to execute `cargo bdd steps`")?;
    assert!(output.status.success(), "`cargo bdd steps` should succeed");
    let stdout =
        str::from_utf8(&output.stdout).wrap_err("`cargo bdd steps` emitted invalid UTF-8")?;
    Ok(stdout.to_string())
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
