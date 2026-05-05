//! Integration tests exercising the `todo-cli` binary.

use assert_cmd::Command;
use predicates::prelude::*;
use rstest_bdd_harness::binary_test_support::{build_binary, workspace_binary_path};
use std::env;
use std::path::PathBuf;

fn locate_or_build_todo_cli_cmd() -> Result<Command, Box<dyn std::error::Error>> {
    match Command::cargo_bin("todo-cli") {
        Ok(command) => Ok(command),
        Err(outer) => {
            let root = workspace_root();
            let binary = workspace_binary_path(&root.join("Cargo.toml"), "todo-cli")?;
            if !binary.is_file() {
                let output = build_binary(&root, "todo-cli")
                    .map_err(|e| format!("failed to spawn cargo build for todo-cli: {e}"))?;
                if !output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    return Err(format!(
                        "todo-cli binary build failed with status {}\nstdout:\n{stdout}\nstderr:\n{stderr}",
                        output.status,
                    )
                    .into());
                }
            }
            if binary.is_file() {
                Ok(Command::new(binary))
            } else {
                Err(Box::new(outer))
            }
        }
    }
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
