//! Utilities for integration-test helpers that locate or build a workspace
//! binary before invoking it.
//!
//! These utilities are exposed as a hidden module for test use only and are
//! not part of the supported public API.

use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Output};

use thiserror::Error;

/// Failure modes for [`locate_or_build_binary`].
#[derive(Debug, Error)]
pub enum BinaryLocateError {
    /// `cargo metadata` could not resolve the workspace target directory.
    #[error(transparent)]
    ManifestLookup(#[from] cargo_metadata::Error),

    /// The `cargo build` subprocess could not be spawned.
    #[error(transparent)]
    CargoSpawn(#[from] std::io::Error),

    /// `cargo build --bin` ran but exited unsuccessfully.
    #[error("`cargo build --bin` failed with status {status}")]
    BuildFailed {
        status: ExitStatus,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    },

    /// The expected binary path exists neither on disk nor after a build.
    #[error("could not locate or build workspace binary at {}", .0.display())]
    MissingBinary(PathBuf),
}

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
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use rstest_bdd_harness::binary_test_support::binary_path_in_target_dir;
///
/// let path = binary_path_in_target_dir(Path::new("/tmp/ws/target"), "my-bin");
/// let suffix = std::env::consts::EXE_SUFFIX;
/// assert_eq!(
///     path,
///     Path::new("/tmp/ws/target/debug").join(format!("my-bin{suffix}"))
/// );
/// ```
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
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
/// use rstest_bdd_harness::binary_test_support::target_directory_for_manifest;
///
/// let target = target_directory_for_manifest(Path::new("Cargo.toml")).expect("metadata");
/// assert!(target.as_path().ends_with("target") || target.to_string_lossy().contains("target"));
/// ```
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
/// # Examples
///
/// ```no_run
/// use std::path::Path;
/// use rstest_bdd_harness::binary_test_support::locate_or_build_binary;
///
/// let cmd = locate_or_build_binary(Path::new("Cargo.toml"), Path::new("."), "my-bin")
///     .expect("locate binary");
/// let _ = cmd;
/// ```
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
) -> Result<Command, BinaryLocateError> {
    if let Some(command) = try_command_from_cargo_test_bin_layout(binary_name) {
        return Ok(command);
    }
    let target_dir = target_directory_for_manifest(manifest_path)?;
    let binary = binary_path_in_target_dir(&target_dir, binary_name);
    if !binary.is_file() {
        let output = build_binary(workspace_root, binary_name)?;
        if !output.status.success() {
            return Err(BinaryLocateError::BuildFailed {
                status: output.status,
                stdout: output.stdout,
                stderr: output.stderr,
            });
        }
    }
    if binary.is_file() {
        Ok(Command::new(binary))
    } else {
        Err(BinaryLocateError::MissingBinary(binary))
    }
}

/// Builds `binary_name` via `cargo build --bin <name>` in `workspace_root`.
///
/// Returns the captured `Output` so callers can include stdout/stderr in
/// diagnostics. Returns `Err` only when the subprocess cannot be spawned.
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
/// use rstest_bdd_harness::binary_test_support::build_binary;
///
/// let output = build_binary(Path::new("."), "some-bin").expect("spawn cargo");
/// assert!(output.status.success() || !output.stderr.is_empty());
/// ```
pub fn build_binary(workspace_root: &Path, binary_name: &str) -> std::io::Result<Output> {
    let cargo = option_env!("CARGO").unwrap_or("cargo");
    Command::new(cargo)
        .current_dir(workspace_root)
        .args(["build", "--bin", binary_name])
        .output()
}

#[cfg(test)]
mod tests {
    //! Unit tests for [`super::binary_path_in_target_dir`],
    //! [`super::target_directory_for_manifest`], [`super::build_binary`], and
    //! [`super::locate_or_build_binary`].

    use std::path::Path;

    use super::{
        binary_path_in_target_dir, build_binary, locate_or_build_binary,
        target_directory_for_manifest,
    };

    // ── binary_path_in_target_dir ──────────────────────────────────────────

    #[test]
    fn binary_path_appends_debug_and_exe_suffix() {
        let target = Path::new("/workspace/target");
        let path = binary_path_in_target_dir(target, "my-tool");
        let expected_name = format!("my-tool{}", std::env::consts::EXE_SUFFIX);
        assert_eq!(path, target.join("debug").join(expected_name));
    }

    #[test]
    fn binary_path_uses_provided_target_directory() {
        let target = Path::new("/custom/target/dir");
        let path = binary_path_in_target_dir(target, "tool");
        assert!(path.starts_with(target));
    }

    // ── target_directory_for_manifest ────────────────────────────────────────

    #[test]
    fn target_directory_for_invalid_manifest_returns_err() {
        let result = target_directory_for_manifest(Path::new("/nonexistent/Cargo.toml"));
        assert!(
            result.is_err(),
            "expected error for non-existent manifest, got: {result:?}"
        );
    }

    #[test]
    fn target_directory_for_workspace_manifest_returns_ok() {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        let result = target_directory_for_manifest(&manifest);
        let Ok(target_dir) = result else {
            panic!("expected Ok for valid manifest, got: {result:?}");
        };
        assert!(
            target_dir.ends_with("target")
                || target_dir
                    .components()
                    .any(|component| component.as_os_str() == "target"),
            "expected target directory to contain a `target` component, got: {}",
            target_dir.display()
        );
    }

    // ── build_binary ─────────────────────────────────────────────────────────

    #[test]
    fn build_binary_returns_err_for_nonexistent_workspace() {
        let result = build_binary(Path::new("/nonexistent/workspace"), "nonexistent-binary");
        match result {
            Err(_) => {}
            Ok(output) => assert!(
                !output.status.success(),
                "build_binary should not succeed for a nonexistent workspace"
            ),
        }
    }

    #[test]
    fn build_binary_captures_output_on_failure() {
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../");
        match build_binary(&workspace_root, "__nonexistent_binary_xyzzy__") {
            Err(_) => {}
            Ok(output) => {
                assert!(
                    !output.status.success(),
                    "should fail for nonexistent binary"
                );
                let stderr = String::from_utf8_lossy(&output.stderr);
                assert!(
                    !stderr.is_empty(),
                    "expected cargo to emit diagnostic output to stderr"
                );
            }
        }
    }

    // ── locate_or_build_binary ─────────────────────────────────────────────

    #[test]
    fn locate_or_build_binary_returns_err_for_invalid_manifest() {
        let workspace = Path::new("/nonexistent/workspace");
        let manifest = Path::new("/nonexistent/does-not-exist/Cargo.toml");
        let result = locate_or_build_binary(
            manifest,
            workspace,
            "__rstest_bdd_harness_locate_invalid_manifest__",
        );
        assert!(
            result.is_err(),
            "expected error for invalid manifest, got: {result:?}"
        );
    }
}
