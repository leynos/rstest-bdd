//! Basic smoke tests for the cargo-bdd subcommand.

use assert_cmd::Command;
use std::str;

#[test]
fn list_steps_runs() {
    let output = Command::cargo_bin("cargo-bdd")
        .expect("binary exists")
        .current_dir("..")
        .arg("steps")
        .output()
        .expect("runs");
    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).expect("utf8");
    assert!(stdout.contains("an unused step"));
}
