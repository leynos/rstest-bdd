//! Integration tests exercising the `todo-cli` binary.

use assert_cmd::{Command, cargo::cargo_bin_cmd};
use predicates::prelude::*;

/// Builds a command for the package's own `todo-cli` binary.
///
/// Cargo builds a package's binary targets before it runs that package's
/// integration tests and hands the path to the test through
/// `CARGO_BIN_EXE_todo-cli`, so no test body compiles anything. A
/// locate-or-build helper would only add a fallback that compiles the binary
/// inside a timed test, which this package never needs.
fn todo_cli_cmd() -> Command { cargo_bin_cmd!("todo-cli") }

#[test]
fn add_succeeds() {
    todo_cli_cmd()
        .args(["add", "Buy milk"])
        .assert()
        .success()
        .stdout("");
}

#[test]
fn list_is_empty_by_default() { todo_cli_cmd().arg("list").assert().success().stdout("\n"); }

#[test]
fn unknown_subcommand_fails() {
    todo_cli_cmd()
        .arg("bogus")
        .assert()
        .failure()
        .stderr(predicates::str::contains("error").or(predicates::str::contains("Usage")));
}

#[test]
fn add_rejects_blank_description() {
    todo_cli_cmd()
        .args(["add", "   "])
        .assert()
        .failure()
        .stderr(predicates::str::contains("must not be blank"));
}
