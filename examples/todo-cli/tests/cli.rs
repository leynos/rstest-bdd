//! Integration tests exercising the `todo-cli` binary.

use assert_cmd::Command;
use predicates::prelude::*;
use std::env;
use std::path::PathBuf;
use std::process::Command as ProcessCommand;

fn todo_cli_cmd() -> Result<Command, Box<dyn std::error::Error>> {
    match Command::cargo_bin("todo-cli") {
        Ok(command) => Ok(command),
        Err(error) => {
            let binary = todo_cli_binary_path()?;
            if !binary.is_file() {
                build_todo_cli_binary()?;
            }
            if binary.is_file() {
                Ok(Command::new(binary))
            } else {
                Err(Box::new(error))
            }
        }
    }
}

fn todo_cli_binary_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path(workspace_root().join("Cargo.toml"))
        .no_deps()
        .exec()?;
    Ok(metadata
        .target_directory
        .into_std_path_buf()
        .join("debug")
        .join(format!("todo-cli{}", env::consts::EXE_SUFFIX)))
}

fn build_todo_cli_binary() -> Result<(), Box<dyn std::error::Error>> {
    let cargo = option_env!("CARGO").unwrap_or("cargo");
    let status = ProcessCommand::new(cargo)
        .current_dir(workspace_root())
        .args(["build", "--bin", "todo-cli"])
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("todo-cli binary build failed with status {status}").into())
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn add_succeeds() -> Result<(), Box<dyn std::error::Error>> {
    todo_cli_cmd()?
        .args(["add", "Buy milk"])
        .assert()
        .success()
        .stdout("");
    Ok(())
}

#[test]
fn list_is_empty_by_default() -> Result<(), Box<dyn std::error::Error>> {
    todo_cli_cmd()?.arg("list").assert().success().stdout("\n");
    Ok(())
}

#[test]
fn unknown_subcommand_fails() -> Result<(), Box<dyn std::error::Error>> {
    todo_cli_cmd()?
        .arg("bogus")
        .assert()
        .failure()
        .stderr(predicates::str::contains("error").or(predicates::str::contains("Usage")));
    Ok(())
}

#[test]
fn add_rejects_blank_description() -> Result<(), Box<dyn std::error::Error>> {
    todo_cli_cmd()?
        .args(["add", "   "])
        .assert()
        .failure()
        .stderr(predicates::str::contains("must not be blank"));
    Ok(())
}
