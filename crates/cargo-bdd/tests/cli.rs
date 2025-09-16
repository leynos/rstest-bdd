//! Basic smoke tests for the cargo-bdd subcommand.

use assert_cmd::Command;
use eyre::{Context, Result};
use std::str;

#[test]
fn list_steps_runs() -> Result<()> {
    let mut cmd = Command::cargo_bin("cargo-bdd")
        .wrap_err("cargo-bdd binary should exist in this workspace")?;
    let output = cmd
        .current_dir("..")
        .arg("steps")
        .output()
        .wrap_err("failed to execute `cargo bdd steps`")?;
    assert!(output.status.success());
    let stdout =
        str::from_utf8(&output.stdout).wrap_err("`cargo bdd steps` emitted invalid UTF-8")?;
    assert!(
        !stdout.is_empty(),
        "Expected non-empty output from steps command"
    );
    Ok(())
}
