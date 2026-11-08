//! Isolated child-process workspace shared by the invocation contract tests.
//!
//! Each workspace owns a temporary directory with dedicated `home`, `work`,
//! `bin` and XDG configuration roots, plus a copy of the `cargo-bdd` binary.
//! Children run with a cleared environment, so a developer's real
//! configuration cannot change what these tests observe.

use std::{ffi::OsStr, io, process::Command};

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs::Dir};
use rstest::fixture;
use tempfile::TempDir;

/// One child invocation: its arguments and any extra environment variables.
///
/// Grouping the two keeps call sites from drifting into a long list of bare
/// string parameters, and lets a case describe a whole configuration source in
/// one value.
#[derive(Clone, Copy)]
pub(super) struct Invocation<'a> {
    /// Arguments passed after the program name.
    pub(super) arguments: &'a [&'a str],
    /// Environment variables set in addition to the isolated defaults.
    pub(super) variables: &'a [(&'a str, &'a str)],
}

/// A temporary directory tree and binary copy for one child-process test.
pub(super) struct IsolatedWorkspace {
    /// Owns the temporary directory for as long as the workspace lives.
    pub(super) temporary_directory: TempDir,
    /// Capability scoped to the temporary directory root.
    pub(super) root: Dir,
    /// Home directory the child treats as the user's home.
    home: Utf8PathBuf,
    /// Package directory the child runs inside.
    working_directory: Utf8PathBuf,
    /// Directory holding the copied binary.
    binary_directory: Utf8PathBuf,
}

impl IsolatedWorkspace {
    /// Build the directory tree and copy the binary under test into it.
    pub(super) fn new() -> io::Result<Self> {
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

    /// Build a child command with the workspace's isolated environment.
    ///
    /// The inherited `PATH` is preserved after the copied binary's directory,
    /// because the child shells out to `cargo` for registry discovery.
    pub(super) fn command(&self, program: impl AsRef<OsStr>) -> io::Result<Command> {
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
            .env("CARGO_NET_OFFLINE", "true")
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

    /// Run the binary by absolute path with the given arguments.
    pub(super) fn direct(&self, args: &[&str]) -> io::Result<std::process::Output> {
        let mut command = self.command(env!("CARGO_BIN_EXE_cargo-bdd"))?;
        command.args(args);
        command.output()
    }

    /// Run the binary with the invocation's arguments and environment.
    pub(super) fn direct_with_environment(
        &self,
        invocation: Invocation<'_>,
    ) -> io::Result<std::process::Output> {
        let mut command = self.command(env!("CARGO_BIN_EXE_cargo-bdd"))?;
        command.args(invocation.arguments);
        for (key, value) in invocation.variables {
            command.env(key, value);
        }
        command.output()
    }

    /// Run the binary as an unprivileged user.
    ///
    /// Some hosts run tests as root, which bypasses the permission bits an
    /// unreadable-configuration test depends on. Dropping to an unprivileged
    /// uid makes the child observe the restriction regardless of the runner.
    #[cfg(target_os = "linux")]
    pub(super) fn direct_as_unprivileged(&self, args: &[&str]) -> io::Result<std::process::Output> {
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

    /// Run the binary through Cargo's external-subcommand dispatch.
    pub(super) fn cargo_dispatch(&self, args: &[&str]) -> io::Result<std::process::Output> {
        let mut command = self.command(env!("CARGO"))?;
        command.args(["bdd"]);
        command.args(args);
        command.output()
    }

    /// Write the working directory's `.cargo_bdd.toml`.
    pub(super) fn write_local_config(&self, contents: &str) -> io::Result<()> {
        let working_directory = self.root.open_dir("work")?;
        working_directory.write(".cargo_bdd.toml", contents)
    }

    /// Write a minimal package with no scenarios into the working directory.
    pub(super) fn write_empty_package(&self) -> io::Result<()> {
        let working_directory = self.root.open_dir("work")?;
        working_directory.create_dir("src")?;
        working_directory.write(
            "Cargo.toml",
            concat!(
                "[package]\n",
                "name = \"empty-cli-fixture\"\n",
                "version = \"0.1.0\"\n",
                "edition = \"2021\"\n\n",
                "[[bin]]\n",
                "name = \"empty-cli-fixture\"\n",
                "path = \"src/main.rs\"\n"
            ),
        )?;
        let source_directory = working_directory.open_dir("src")?;
        source_directory.write("main.rs", "fn main() {}\n")
    }
}

/// Create an isolated workspace, or the I/O error that prevented it.
#[rstest_bdd_test_macros::allow_fixture_expansion_lints]
#[fixture]
pub(super) fn isolated_workspace() -> io::Result<IsolatedWorkspace> { IsolatedWorkspace::new() }
