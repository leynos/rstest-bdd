use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn no_args_prints_help() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("cargo-bdd")?;
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("list-steps"));
    Ok(())
}

#[test]
fn list_steps_outputs_registered_steps() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("cargo-bdd")?;
    cmd.arg("list-steps");
    cmd.assert().success().stdout(
        predicate::str::contains("Given I have cukes")
            .and(predicate::str::contains("When I eat them"))
            .and(predicate::str::contains("Then I should be satisfied")),
    );
    Ok(())
}

#[test]
fn list_duplicates_reports_duplicates() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("cargo-bdd")?;
    cmd.arg("list-duplicates");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("duplicate step"));
    Ok(())
}

#[test]
fn list_unused_shows_unused_definitions() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("cargo-bdd")?;
    cmd.args(["list-unused", "tests/features"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("unused step"));
    Ok(())
}
