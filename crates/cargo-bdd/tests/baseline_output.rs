//! Freeze the existing no-configuration command-line contract.

use std::{
    error::Error,
    io,
    process::{Command, Output},
};

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs::Dir};
use serde::Deserialize;
use tempfile::TempDir;

#[derive(Debug, Deserialize)]
struct BaselineCase {
    name: String,
    args: Vec<String>,
    exit_code: i32,
    stdout: String,
    stderr: String,
}

struct IsolatedConfigDirectories {
    _temporary_directory: TempDir,
    home: Utf8PathBuf,
    xdg_config_home: Utf8PathBuf,
    xdg_config_dirs: Utf8PathBuf,
    user_profile: Utf8PathBuf,
    appdata: Utf8PathBuf,
    working_directory: Utf8PathBuf,
}

impl IsolatedConfigDirectories {
    fn new() -> io::Result<Self> {
        let temporary_directory = tempfile::tempdir()?;
        let root = Utf8Path::from_path(temporary_directory.path())
            .ok_or_else(|| io::Error::other("temporary directory path is not UTF-8"))?;
        create_config_directories(root)?;
        let home = root.join("home");
        let xdg_config_home = root.join("xdg-config-home");
        let xdg_config_dirs = root.join("xdg-config-dirs");
        let user_profile = root.join("user-profile");
        let appdata = root.join("appdata");
        let working_directory = root.join("working-directory");

        Ok(Self {
            _temporary_directory: temporary_directory,
            home,
            xdg_config_home,
            xdg_config_dirs,
            user_profile,
            appdata,
            working_directory,
        })
    }
}

fn create_config_directories(root: &Utf8Path) -> io::Result<()> {
    let directory = Dir::open_ambient_dir(root.as_std_path(), ambient_authority())?;
    for path in [
        "home",
        "xdg-config-home",
        "xdg-config-dirs",
        "user-profile",
        "appdata",
        "working-directory",
    ] {
        directory.create_dir(path)?;
    }
    Ok(())
}

#[test]
fn cli_without_configuration_matches_the_captured_baseline() -> Result<(), Box<dyn Error>> {
    let cases = baseline_cases()?;
    let isolated_directories = IsolatedConfigDirectories::new()?;

    for case in cases {
        assert_case_matches_baseline(&case, &isolated_directories)?;
    }
    Ok(())
}

fn baseline_cases() -> Result<Vec<BaselineCase>, serde_json::Error> {
    serde_json::from_str(include_str!("fixtures/baseline-cli.json"))
}

fn assert_case_matches_baseline(
    case: &BaselineCase,
    directories: &IsolatedConfigDirectories,
) -> io::Result<()> {
    let output = run_cli_case(case, directories)?;
    assert_eq!(
        output.status.code(),
        Some(case.exit_code),
        "unexpected exit status for {}",
        case.name
    );
    assert_eq!(
        output.stdout,
        expected_output(&case.stdout),
        "unexpected stdout for {}",
        case.name
    );
    assert_eq!(
        output.stderr,
        expected_output(&case.stderr),
        "unexpected stderr for {}",
        case.name
    );
    Ok(())
}

fn run_cli_case(
    case: &BaselineCase,
    directories: &IsolatedConfigDirectories,
) -> io::Result<Output> {
    Command::new(env!("CARGO_BIN_EXE_cargo-bdd"))
        .args(&case.args)
        .env_clear()
        .env("HOME", &directories.home)
        .env("XDG_CONFIG_HOME", &directories.xdg_config_home)
        .env("XDG_CONFIG_DIRS", &directories.xdg_config_dirs)
        .env("USERPROFILE", &directories.user_profile)
        .env("APPDATA", &directories.appdata)
        // Registry discovery runs `cargo metadata`; keep this child offline.
        .env("CARGO_NET_OFFLINE", "true")
        .current_dir(&directories.working_directory)
        .output()
}

fn expected_output(template: &str) -> Vec<u8> {
    // `help <command>` retains the root name; subcommand parse errors do not.
    template
        .replace("<CARGO_BDD_ROOT_BINARY>", expected_root_binary_name())
        .replace("<CARGO_BDD_SUBCOMMAND_BINARY>", "cargo-bdd")
        .replace("<CARGO_PKG_VERSION>", env!("CARGO_PKG_VERSION"))
        .into_bytes()
}

fn expected_root_binary_name() -> &'static str {
    if cfg!(windows) {
        "cargo-bdd.exe"
    } else {
        "cargo-bdd"
    }
}
