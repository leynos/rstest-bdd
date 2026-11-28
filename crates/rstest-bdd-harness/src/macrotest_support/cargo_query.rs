//! The `cargo metadata` query `macrotest_support` resolves its target root with.
//!
//! Kept in a module of its own so the query's argument list and environment
//! are one thing to assert, and so the parent module stays within the
//! repository's per-file line budget.

use std::path::Path;

/// Build the `cargo metadata` query that resolves the trybuild target root.
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

#[cfg(test)]
mod tests {
    //! Assertions on the query's argument list and environment.

    use std::{ffi::OsStr, path::Path};

    use super::metadata_command;

    #[test]
    fn the_target_root_query_is_offline_and_skips_dependencies() {
        // The query reads only `target_directory`. Resolving the dependency
        // graph would be pure cost, and an online resolution would make a
        // preparation stamp depend on the network — the state this suite is
        // meant to avoid.
        let command = metadata_command(Path::new("Cargo.toml"));
        let cargo = command.cargo_command();
        let args = cargo
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        let offline = cargo
            .get_envs()
            .find(|(key, _)| *key == OsStr::new("CARGO_NET_OFFLINE"))
            .and_then(|(_, value)| value);

        assert!(
            args.iter().any(|arg| arg == "--no-deps"),
            "the target-root query must not resolve dependencies; got {args:?}"
        );
        assert_eq!(
            offline,
            Some(OsStr::new("true")),
            "the target-root query must run offline"
        );
    }
}
