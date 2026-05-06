//! Utilities for integration-test helpers that locate or build a workspace
//! binary before invoking it.
//!
//! These utilities are exposed as a hidden module for test use only and are
//! not part of the supported public API.

use std::env;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::Command;

// Adapted from assert_cmd's cargo helper: same `CARGO_BIN_EXE_<name>` convention
// and `current_exe`-derived target-dir fallback.
fn cargo_bin_path_for_integration_tests(binary_name: &str) -> PathBuf {
    let env_var = format!("CARGO_BIN_EXE_{binary_name}");
    env::var_os(&env_var).map_or_else(
        || {
            target_dir_near_current_exe()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(format!("{binary_name}{}", env::consts::EXE_SUFFIX))
        },
        PathBuf::from,
    )
}

fn target_dir_near_current_exe() -> Option<PathBuf> {
    let mut path = env::current_exe().ok()?;
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    Some(path)
}

fn try_command_from_cargo_test_bin_layout(binary_name: &str) -> Option<Command> {
    let path = cargo_bin_path_for_integration_tests(binary_name);
    path.is_file().then(|| Command::new(path))
}

/// Returns the expected debug binary path for `binary_name` given a target
/// directory root.
///
/// Pure computation: does not invoke `cargo` or perform any I/O.
#[must_use]
pub fn binary_path_in_target_dir(target_directory: &Path, binary_name: &str) -> PathBuf {
    target_directory
        .join("debug")
        .join(format!("{binary_name}{}", env::consts::EXE_SUFFIX))
}

/// Resolves the workspace target directory by running `cargo metadata` for the
/// given manifest path.
///
/// Performs I/O: spawns a `cargo metadata` subprocess.
pub fn target_directory_for_manifest(
    manifest_path: &Path,
) -> Result<PathBuf, cargo_metadata::Error> {
    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path(manifest_path)
        .no_deps()
        .exec()?;
    Ok(metadata.target_directory.into_std_path_buf())
}

/// Locates `binary_name` or builds it via `cargo build --bin <name>` if
/// absent. Returns a `std::process::Command` ready to execute the binary.
///
/// Strategy:
/// 1. Try the path implied by `CARGO_BIN_EXE_<binary_name>` (and the same
///    `current_exe` fallback used by `assert_cmd`'s `cargo_bin` helper).
/// 2. On failure, resolve the expected debug binary path via
///    `target_directory_for_manifest`.
/// 3. If the binary is absent, invoke `build_binary` and surface stdout/stderr
///    on failure.
/// 4. Return `Command::new(binary)` when the binary is present.
pub fn locate_or_build_binary(
    manifest_path: &Path,
    workspace_root: &Path,
    binary_name: &str,
) -> Result<Command, Box<dyn Error>> {
    if let Some(command) = try_command_from_cargo_test_bin_layout(binary_name) {
        return Ok(command);
    }
    let target_dir = target_directory_for_manifest(manifest_path)?;
    let binary = binary_path_in_target_dir(&target_dir, binary_name);
    if !binary.is_file() {
        let output = build_binary(workspace_root, binary_name)
            .map_err(|e| format!("failed to spawn `cargo build --bin {binary_name}`: {e}"))?;
        if !output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "`cargo build --bin {binary_name}` failed with status {}\nstdout:\n{stdout}\nstderr:\n{stderr}",
                output.status,
            )
            .into());
        }
    }
    if binary.is_file() {
        Ok(Command::new(binary))
    } else {
        Err(format!(
            "could not locate or build workspace binary `{binary_name}` at {}",
            binary.display(),
        )
        .into())
    }
}

/// Builds `binary_name` via `cargo build --bin <name>` in `workspace_root`.
///
/// Returns the captured `Output` so callers can include stdout/stderr in
/// diagnostics. Returns `Err` only when the subprocess cannot be spawned.
pub fn build_binary(
    workspace_root: &Path,
    binary_name: &str,
) -> std::io::Result<std::process::Output> {
    let cargo = option_env!("CARGO").unwrap_or("cargo");
    Command::new(cargo)
        .current_dir(workspace_root)
        .args(["build", "--bin", binary_name])
        .output()
}
