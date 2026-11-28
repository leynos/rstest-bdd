//! The two Cargo processes `binary_test_support` spawns.
//!
//! Both are built here rather than at their call sites so their argument lists
//! and environment are one thing to assert: a metadata query that resolves
//! dependencies, or a build that reaches the network, would make a test run
//! depend on state the prefetch step does not guarantee.

use std::{path::Path, process::Command};

use super::BinaryName;

/// Build the `cargo metadata` query that resolves the workspace target root.
///
/// Only the target directory is read, so `--no-deps` avoids a resolve graph
/// the caller never inspects, and offline mode keeps a cold cache from turning
/// the lookup into a registry request.
pub(super) fn metadata_command(manifest_path: &Path) -> cargo_metadata::MetadataCommand {
    let mut command = cargo_metadata::MetadataCommand::new();
    command
        .manifest_path(manifest_path)
        .no_deps()
        .env("CARGO_NET_OFFLINE", "true");
    command
}

/// Build the `cargo build --bin` invocation for `binary_name`.
///
/// `--locked` keeps the build from re-resolving the commit-pinned graph and
/// `--offline` keeps a test-run build off the network; a nested build that
/// needs either is a fixture the prefetch step should have covered.
pub(super) fn build_command(workspace_root: &Path, binary_name: BinaryName<'_>) -> Command {
    let cargo = option_env!("CARGO").unwrap_or("cargo");
    let mut command = Command::new(cargo);
    command
        .current_dir(workspace_root)
        .env("CARGO_NET_OFFLINE", "true")
        .args([
            "build",
            "--locked",
            "--offline",
            "--bin",
            binary_name.as_str(),
        ]);
    command
}

#[cfg(test)]
mod tests {
    //! Assertions on each command's argument list and environment.

    use std::{ffi::OsStr, path::Path, process::Command};

    use super::{build_command, metadata_command};
    use crate::binary_test_support::BinaryName;

    fn offline_value(command: &Command) -> Option<std::ffi::OsString> {
        command
            .get_envs()
            .find(|(key, _)| *key == OsStr::new("CARGO_NET_OFFLINE"))
            .and_then(|(_, value)| value.map(OsStr::to_os_string))
    }

    fn args_of(command: &Command) -> Vec<String> {
        command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn metadata_query_is_offline_and_skips_dependencies() {
        let command = metadata_command(Path::new("Cargo.toml"));
        let cargo = command.cargo_command();
        let args = args_of(&cargo);

        assert!(
            args.iter().any(|arg| arg == "--no-deps"),
            "the target-root query must not resolve dependencies; got {args:?}"
        );
        assert_eq!(
            offline_value(&cargo).as_deref(),
            Some(OsStr::new("true")),
            "the target-root query must run offline"
        );
    }

    #[test]
    fn build_command_is_locked_and_offline() {
        let command = build_command(Path::new("/workspace"), BinaryName::new("cargo-bdd"));
        let args = args_of(&command);

        assert!(
            args.iter().any(|arg| arg == "--locked"),
            "a test-run build must not re-resolve the lockfile; got {args:?}"
        );
        assert!(
            args.iter().any(|arg| arg == "--offline"),
            "a test-run build must not reach the network; got {args:?}"
        );
        assert_eq!(
            offline_value(&command).as_deref(),
            Some(OsStr::new("true")),
            "a test-run build must be told offline mode explicitly"
        );
    }
}
