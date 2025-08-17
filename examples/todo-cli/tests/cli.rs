use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn add_succeeds() -> Result<(), Box<dyn std::error::Error>> {
    Command::cargo_bin("todo-cli")?
        .args(["add", "Buy milk"])
        .assert()
        .success()
        .stdout("");
    Ok(())
}

#[test]
fn list_is_empty_by_default() -> Result<(), Box<dyn std::error::Error>> {
    Command::cargo_bin("todo-cli")?
        .arg("list")
        .assert()
        .success()
        .stdout("\n");
    Ok(())
}

#[test]
fn unknown_subcommand_fails() -> Result<(), Box<dyn std::error::Error>> {
    Command::cargo_bin("todo-cli")?
        .arg("bogus")
        .assert()
        .failure()
        .stderr(predicates::str::contains("error").or(predicates::str::contains("Usage")));
    Ok(())
}

#[test]
fn add_rejects_blank_description() -> Result<(), Box<dyn std::error::Error>> {
    Command::cargo_bin("todo-cli")?
        .args(["add", "   "])
        .assert()
        .failure()
        .stderr(predicates::str::contains("must not be blank"));
    Ok(())
}
