use assert_cmd::Command;

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
