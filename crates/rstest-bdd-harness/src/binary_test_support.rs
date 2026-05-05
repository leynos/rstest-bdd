//! Utilities for integration-test helpers that locate or build a workspace
//! binary before invoking it.
//!
//! These utilities are exposed as a hidden module for test use only and are
//! not part of the supported public API.

use std::env;
use std::path::PathBuf;
use std::process::Command;

/// Returns the expected debug binary path for `binary_name` using
/// `cargo_metadata` to resolve the workspace target directory.
///
/// Does not invoke `cargo`; the binary may not yet exist.
pub fn workspace_binary_path(
    manifest_path: &std::path::Path,
    binary_name: &str,
) -> Result<PathBuf, cargo_metadata::Error> {
    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path(manifest_path)
        .no_deps()
        .exec()?;
    Ok(metadata
        .target_directory
        .into_std_path_buf()
        .join("debug")
        .join(format!("{binary_name}{}", env::consts::EXE_SUFFIX)))
}

/// Builds `binary_name` via `cargo build --bin <name>` in `workspace_root`.
///
/// Returns the captured `Output` so callers can include stdout/stderr in
/// diagnostics. Returns `Err` only when the subprocess cannot be spawned.
pub fn build_binary(
    workspace_root: &std::path::Path,
    binary_name: &str,
) -> std::io::Result<std::process::Output> {
    let cargo = option_env!("CARGO").unwrap_or("cargo");
    Command::new(cargo)
        .current_dir(workspace_root)
        .args(["build", "--bin", binary_name])
        .output()
}
