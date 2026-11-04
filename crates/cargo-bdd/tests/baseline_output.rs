//! Freeze the existing no-configuration command-line contract.

use std::process::Command;

use camino::Utf8Path;
use cap_std::{ambient_authority, fs::Dir};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct BaselineCase {
    name: String,
    args: Vec<String>,
    exit_code: i32,
    stdout: String,
    stderr: String,
}

#[test]
fn cli_without_configuration_matches_the_captured_baseline() {
    let cases: Vec<BaselineCase> = serde_json::from_str(include_str!("fixtures/baseline-cli.json"))
        .expect("baseline fixture should be valid JSON");
    let isolation = tempfile::tempdir().expect("temporary directory should be created");
    let isolation_root = Utf8Path::from_path(isolation.path())
        .expect("temporary directory should be representable as UTF-8");
    let isolation_directory =
        Dir::open_ambient_dir(isolation_root.as_std_path(), ambient_authority())
            .expect("test setup should open an isolated directory capability");
    for directory in [
        "home",
        "xdg-config-home",
        "xdg-config-dirs",
        "user-profile",
        "appdata",
        "working-directory",
    ] {
        isolation_directory
            .create_dir(directory)
            .expect("isolated config directory should be created");
    }
    let home = isolation_root.join("home");
    let xdg_config_home = isolation_root.join("xdg-config-home");
    let xdg_config_dirs = isolation_root.join("xdg-config-dirs");
    let user_profile = isolation_root.join("user-profile");
    let appdata = isolation_root.join("appdata");
    let working_directory = isolation_root.join("working-directory");

    for case in cases {
        let output = Command::new(env!("CARGO_BIN_EXE_cargo-bdd"))
            .args(&case.args)
            .env_clear()
            .env("HOME", &home)
            .env("XDG_CONFIG_HOME", &xdg_config_home)
            .env("XDG_CONFIG_DIRS", &xdg_config_dirs)
            .env("USERPROFILE", &user_profile)
            .env("APPDATA", &appdata)
            .current_dir(&working_directory)
            .output()
            .expect("cargo-bdd process should start");

        assert_eq!(
            output.status.code(),
            Some(case.exit_code),
            "unexpected exit status for {}",
            case.name
        );
        let binary_name = if cfg!(windows) {
            "cargo-bdd.exe"
        } else {
            "cargo-bdd"
        };
        let expected_stdout = case
            .stdout
            .replace("<CARGO_BDD_BINARY>", binary_name)
            .replace("<CARGO_PKG_VERSION>", env!("CARGO_PKG_VERSION"));
        let expected_stderr = case.stderr.replace("<CARGO_BDD_BINARY>", binary_name);
        assert_eq!(
            output.stdout,
            expected_stdout.as_bytes(),
            "unexpected stdout for {}",
            case.name
        );
        assert_eq!(
            output.stderr,
            expected_stderr.as_bytes(),
            "unexpected stderr for {}",
            case.name
        );
    }
}
