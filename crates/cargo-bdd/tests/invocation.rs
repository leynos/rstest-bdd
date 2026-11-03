//! Binary-level contract for direct and Cargo-dispatched CLI entry points.

use std::{
    io,
    process::{Command, Output},
};

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs::Dir};
use rstest::{fixture, rstest};
use tempfile::TempDir;

struct IsolatedWorkspace {
    temporary_directory: TempDir,
    root: Dir,
    home: Utf8PathBuf,
    working_directory: Utf8PathBuf,
    binary_directory: Utf8PathBuf,
}

impl IsolatedWorkspace {
    fn new() -> io::Result<Self> {
        let temporary_directory = tempfile::tempdir()?;
        let temporary_root = Utf8Path::from_path(temporary_directory.path()).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "test directory path is not UTF-8",
            )
        })?;
        let root = Dir::open_ambient_dir(temporary_root.as_std_path(), ambient_authority())?;
        for directory in ["home", "work", "bin"] {
            root.create_dir(directory)?;
        }
        let home = temporary_root.join("home");
        let working_directory = temporary_root.join("work");
        let binary_directory = temporary_root.join("bin");
        let binary_path = Utf8Path::new(env!("CARGO_BIN_EXE_cargo-bdd"));
        let binary_parent = binary_path.parent().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "Cargo binary has no parent directory",
            )
        })?;
        let source_directory = Dir::open_ambient_dir(binary_parent, ambient_authority())?;
        let destination_directory = root.open_dir("bin")?;
        let binary_name = binary_path.file_name().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "Cargo binary path has no file name",
            )
        })?;
        source_directory.copy(binary_name, &destination_directory, binary_name)?;

        Ok(Self {
            temporary_directory,
            root,
            home,
            working_directory,
            binary_directory,
        })
    }

    fn command(&self, program: impl AsRef<std::ffi::OsStr>) -> io::Result<Command> {
        let temporary_root =
            Utf8Path::from_path(self.temporary_directory.path()).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "test directory path is not UTF-8",
                )
            })?;
        let config_home = temporary_root.join("xdg-config-home");
        let config_dirs = temporary_root.join("xdg-config-dirs");
        self.root.create_dir_all("xdg-config-home")?;
        self.root.create_dir_all("xdg-config-dirs")?;
        let inherited_path = std::env::var_os("PATH").unwrap_or_default();
        let mut path_parts = vec![self.binary_directory.as_os_str().to_owned()];
        path_parts.extend(
            std::env::split_paths(&inherited_path)
                .filter(|path| !path.as_os_str().is_empty())
                .map(std::path::PathBuf::into_os_string),
        );
        let path = std::env::join_paths(path_parts)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
        let mut command = Command::new(program);
        command
            .env_clear()
            .env("HOME", self.home.as_os_str())
            .env("USERPROFILE", self.home.as_os_str())
            .env("XDG_CONFIG_HOME", config_home.as_os_str())
            .env("XDG_CONFIG_DIRS", config_dirs.as_os_str())
            .env("APPDATA", self.home.as_os_str())
            .env("PATH", path)
            .env("CARGO", env!("CARGO"))
            .current_dir(&self.working_directory);
        if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
            let home = Utf8PathBuf::try_from(home)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
            command
                .env(
                    "CARGO_HOME",
                    std::env::var_os("CARGO_HOME")
                        .unwrap_or_else(|| home.join(".cargo").into_os_string()),
                )
                .env(
                    "RUSTUP_HOME",
                    std::env::var_os("RUSTUP_HOME")
                        .unwrap_or_else(|| home.join(".rustup").into_os_string()),
                );
        }
        if let Some(system_root) = std::env::var_os("SystemRoot") {
            command.env("SystemRoot", system_root);
        }
        Ok(command)
    }

    fn direct(&self, args: &[&str]) -> io::Result<Output> {
        let mut command = self.command(env!("CARGO_BIN_EXE_cargo-bdd"))?;
        command.args(args);
        command.output()
    }

    #[cfg(target_os = "linux")]
    fn direct_as_unprivileged(&self, args: &[&str]) -> io::Result<Output> {
        use std::os::unix::process::CommandExt;

        use cap_std::fs::{Permissions, PermissionsExt};

        let binary = self.binary_directory.join("cargo-bdd");
        let mut command = self.command(binary.as_str())?;
        for directory in [
            ".",
            "home",
            "work",
            "bin",
            "xdg-config-home",
            "xdg-config-dirs",
        ] {
            self.root
                .set_permissions(directory, Permissions::from_mode(0o755))?;
        }
        let binaries = self.root.open_dir("bin")?;
        binaries.set_permissions("cargo-bdd", Permissions::from_mode(0o755))?;
        command.args(args);
        command.uid(65_534);
        command.output()
    }

    fn cargo_dispatch(&self, args: &[&str]) -> io::Result<Output> {
        let mut command = self.command(env!("CARGO"))?;
        command.args(["bdd"]);
        command.args(args);
        command.output()
    }

    fn write_local_config(&self, contents: &str) -> io::Result<()> {
        let working_directory = self.root.open_dir("work")?;
        working_directory.write(".cargo_bdd.toml", contents)
    }
}

#[rstest_bdd_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn isolated_workspace() -> io::Result<IsolatedWorkspace> { IsolatedWorkspace::new() }

#[rstest]
fn flat_injected_and_real_cargo_forms_share_the_same_command_help(
    isolated_workspace: io::Result<IsolatedWorkspace>,
) -> io::Result<()> {
    let workspace = isolated_workspace?;
    let flat = workspace.direct(&["steps", "--help"])?;
    let injected = workspace.direct(&["bdd", "steps", "--help"])?;
    let cargo = workspace.cargo_dispatch(&["steps", "--help"])?;

    assert!(flat.status.success(), "flat invocation should succeed");
    assert!(
        injected.status.success(),
        "injected invocation should succeed"
    );
    assert!(cargo.status.success(), "Cargo dispatch should succeed");
    assert!(String::from_utf8_lossy(&flat.stdout).contains("Usage: cargo-bdd steps"));
    assert!(String::from_utf8_lossy(&injected.stdout).contains("Usage: cargo bdd steps"));
    assert_eq!(injected.stdout, cargo.stdout);
    assert_eq!(injected.stderr, cargo.stderr);
    assert_eq!(flat.stderr, [] as [u8; 0]);
    Ok(())
}

#[rstest]
fn injected_wrapper_keeps_unknown_inner_commands_as_parse_errors(
    isolated_workspace: io::Result<IsolatedWorkspace>,
) -> io::Result<()> {
    let workspace = isolated_workspace?;
    let output = workspace.direct(&["bdd", "unknown-command"])?;

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown-command"));
    Ok(())
}

#[rstest]
#[case::malformed_toml("[cmds.steps\nskipped = true\n")]
#[case::invalid_value_type("[cmds.steps]\nskipped = \"perhaps\"\n")]
fn configuration_file_errors_exit_one_on_stderr(
    isolated_workspace: io::Result<IsolatedWorkspace>,
    #[case] contents: &str,
) -> io::Result<()> {
    let workspace = isolated_workspace?;
    workspace.write_local_config(contents)?;

    let output = workspace.direct(&["steps"])?;

    assert_configuration_error(&output);
    Ok(())
}

#[cfg(target_os = "linux")]
#[rstest]
fn unreadable_local_configuration_exits_one_on_stderr(
    isolated_workspace: io::Result<IsolatedWorkspace>,
) -> io::Result<()> {
    use cap_std::fs::{Permissions, PermissionsExt};

    let workspace = isolated_workspace?;
    workspace.write_local_config("[cmds.steps]\nskipped = true\n")?;
    let working_directory = workspace.root.open_dir("work")?;
    working_directory.set_permissions(".cargo_bdd.toml", Permissions::from_mode(0o000))?;
    let current_user_can_read = working_directory.open(".cargo_bdd.toml").is_ok();
    let output = if current_user_can_read {
        workspace.direct_as_unprivileged(&["steps"])?
    } else {
        workspace.direct(&["steps"])?
    };

    assert_configuration_error(&output);
    Ok(())
}

#[cfg(all(unix, not(target_os = "linux")))]
#[rstest]
fn unreadable_local_configuration_exits_one_on_stderr(
    isolated_workspace: io::Result<IsolatedWorkspace>,
) -> io::Result<()> {
    use cap_std::fs::{Permissions, PermissionsExt};

    let workspace = isolated_workspace?;
    workspace.write_local_config("[cmds.steps]\nskipped = true\n")?;
    let working_directory = workspace.root.open_dir("work")?;
    working_directory.set_permissions(".cargo_bdd.toml", Permissions::from_mode(0o000))?;
    if working_directory.open(".cargo_bdd.toml").is_ok() {
        // Some privileged Unix runners can bypass file permission bits.
        return Ok(());
    }

    let output = workspace.direct(&["steps"])?;

    assert_configuration_error(&output);
    Ok(())
}

#[cfg(windows)]
#[rstest]
fn unreadable_local_configuration_exits_one_on_stderr(
    isolated_workspace: io::Result<IsolatedWorkspace>,
) -> io::Result<()> {
    use cap_std::fs::{OpenOptions, OpenOptionsExt};

    let workspace = isolated_workspace?;
    workspace.write_local_config("[cmds.steps]\nskipped = true\n")?;
    let working_directory = workspace.root.open_dir("work")?;
    let mut options = OpenOptions::new();
    options.write(true).share_mode(0);
    let _exclusive_file = working_directory.open_with(".cargo_bdd.toml", &options)?;

    let output = workspace.direct(&["steps"])?;

    assert_configuration_error(&output);
    Ok(())
}

fn assert_configuration_error(output: &Output) {
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(output.stdout, [] as [u8; 0]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("failed to load configuration for `steps`"));
}
