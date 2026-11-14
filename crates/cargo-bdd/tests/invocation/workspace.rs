//! Isolated child-process workspace shared by the invocation contract tests.
//!
//! Each workspace owns a temporary directory with dedicated `home`, `work`,
//! `bin` and XDG configuration roots, plus a copy of the `cargo-bdd` binary.
//! Children run with a cleared environment, so a developer's real
//! configuration cannot change what these tests observe.
//!
//! The one inherited variable is `LLVM_PROFILE_FILE`. The copied binary is the
//! process under test, so `cargo llvm-cov` merges its profile only if the child
//! writes to the pattern the coverage driver exported. Clearing it does not
//! fail anything — the run stays green and the lines the binary executed report
//! as never covered — so the passthrough is the difference between a measured
//! binary and an unmeasured one.

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
        for home in ToolchainHome::ALL {
            if let Some(value) = home.explicit_or_derived()? {
                command.env(home.variable(), value);
            }
        }
        if let Some(system_root) = std::env::var_os("SystemRoot") {
            command.env("SystemRoot", system_root);
        }
        // The child is the process under test, so it keeps the coverage
        // driver's profile pattern; redirecting it would drop this binary's
        // exercised lines from the run that measures them.
        if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
            command.env("LLVM_PROFILE_FILE", profile);
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

/// One toolchain home the cleared environment has to carry across.
///
/// The variable name and the directory it defaults to are the same fact, so
/// they are one value rather than a pair of parallel string parameters that a
/// caller could transpose.
#[derive(Clone, Copy)]
enum ToolchainHome {
    /// Cargo's registry and cache home.
    Cargo,
    /// Rustup's toolchain home.
    Rustup,
}

impl ToolchainHome {
    /// Both homes, in the order they are exported.
    const ALL: [Self; 2] = [Self::Cargo, Self::Rustup];

    /// The environment variable that names this home.
    fn variable(self) -> &'static str {
        match self {
            Self::Cargo => "CARGO_HOME",
            Self::Rustup => "RUSTUP_HOME",
        }
    }

    /// The directory appended to an ambient home when the variable is unset.
    fn default_directory(self) -> &'static str {
        match self {
            Self::Cargo => ".cargo",
            Self::Rustup => ".rustup",
        }
    }

    /// Resolve this home for the cleared environment.
    ///
    /// The child's environment is cleared, so an explicit `CARGO_HOME` or
    /// `RUSTUP_HOME` has to be re-exported here or it is lost. Each is
    /// resolved on its own: an explicit value needs no ambient home, and
    /// requiring one would drop an override that the parent process did set.
    /// Only the fallback is derived, and only when an ambient home exists.
    ///
    /// That fallback is [`toolchain_home`], not `HOME`, because Cargo and
    /// Rustup resolve their home through the `home` crate: on Windows that is
    /// `USERPROFILE` and then the profile Known Folder API, and it never reads
    /// `HOME`. Configuring both from `HOME` would point this child at a
    /// registry and toolchain that do not exist whenever a Windows shell sets
    /// a Unix-style `HOME` distinct from the profile, and every
    /// Cargo-dispatched invocation case would then fail to find the installed
    /// toolchain.
    ///
    /// A value that is not valid UTF-8 is an error rather than a lossily
    /// encoded path, which would silently name a different directory.
    fn explicit_or_derived(self) -> io::Result<Option<std::ffi::OsString>> {
        if let Some(explicit) = std::env::var_os(self.variable()) {
            return Ok(Some(explicit));
        }
        let Some(home) = toolchain_home() else {
            return Ok(None);
        };
        let home = Utf8PathBuf::try_from(home)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        Ok(Some(home.join(self.default_directory()).into_os_string()))
    }
}

/// The home Cargo and Rustup derive their defaults from.
///
/// An empty value counts as absent on both platforms: `home` treats an empty
/// `USERPROFILE` that way, and an empty `HOME` would otherwise derive `.cargo`
/// relative to the child's working directory rather than to any home. The
/// platform order is the one `home` documents, not the order the configuration
/// file's own discovery uses; see [`ToolchainHome::explicit_or_derived`].
fn toolchain_home() -> Option<std::ffi::OsString> {
    let names: &[&str] = if cfg!(windows) {
        &["USERPROFILE", "HOME"]
    } else {
        &["HOME", "USERPROFILE"]
    };
    names
        .iter()
        .find_map(|name| std::env::var_os(name).filter(|value| !value.is_empty()))
}

/// Create an isolated workspace, or the I/O error that prevented it.
#[rstest_bdd_test_macros::allow_fixture_expansion_lints]
#[fixture]
pub(super) fn isolated_workspace() -> io::Result<IsolatedWorkspace> { IsolatedWorkspace::new() }
