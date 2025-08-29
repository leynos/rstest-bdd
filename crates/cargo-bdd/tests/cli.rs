//! Basic smoke tests for the cargo-bdd subcommand.

use assert_cmd::Command;

#[test]
fn list_steps_runs() {
    Command::cargo_bin("cargo-bdd")
        .expect("binary exists")
        .args(["steps"])
        .assert()
        .success();
}
