//! Integration tests exercising the `todo-cli` binary.

use assert_cmd::Command;
use predicates::prelude::*;
use rstest_bdd_harness::binary_test_support::{BinaryName, locate_or_build_binary};
use std::env;
use std::path::PathBuf;

fn locate_or_build_todo_cli_cmd() -> Result<Command, Box<dyn std::error::Error>> {
    let root = workspace_root();
    locate_or_build_binary(&root.join("Cargo.toml"), &root, BinaryName::new("todo-cli"))
        .map(Command::from_std)
        .map_err(|e| -> Box<dyn std::error::Error> { Box::new(e) })
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn add_succeeds() -> Result<(), Box<dyn std::error::Error>> {
    locate_or_build_todo_cli_cmd()?
        .args(["add", "Buy milk"])
        .assert()
        .success()
        .stdout("");
    Ok(())
}

#[test]
fn list_is_empty_by_default() -> Result<(), Box<dyn std::error::Error>> {
    locate_or_build_todo_cli_cmd()?
        .arg("list")
        .assert()
        .success()
        .stdout("\n");
    Ok(())
}

#[test]
fn unknown_subcommand_fails() -> Result<(), Box<dyn std::error::Error>> {
    locate_or_build_todo_cli_cmd()?
        .arg("bogus")
        .assert()
        .failure()
        .stderr(predicates::str::contains("error").or(predicates::str::contains("Usage")));
    Ok(())
}

#[test]
fn add_rejects_blank_description() -> Result<(), Box<dyn std::error::Error>> {
    locate_or_build_todo_cli_cmd()?
        .args(["add", "   "])
        .assert()
        .failure()
        .stderr(predicates::str::contains("must not be blank"));
    Ok(())
}
