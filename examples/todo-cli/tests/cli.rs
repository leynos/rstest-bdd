use assert_cmd::Command;

#[test]
fn add_then_list_is_empty() -> Result<(), Box<dyn std::error::Error>> {
    Command::cargo_bin("todo-cli")?
        .args(["add", "Buy milk"])
        .assert()
        .success();

    Command::cargo_bin("todo-cli")?
        .arg("list")
        .assert()
        .success()
        .stdout("\n");
    Ok(())
}
