//! Utilities for integration-test helpers that locate or build a workspace
//! binary before invoking it.
//!
//! These utilities are exposed as a hidden module for test use only and are
//! not part of the supported public API.

use std::{
    env,
    fmt,
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Output},
};

use thiserror::Error;

/// The name of a workspace binary as passed to `cargo build --bin`.
///
/// Wraps `&str` to distinguish binary names from arbitrary string arguments
/// at the call site and to reduce primitive obsession on `&str`.
#[derive(Debug, Clone, Copy)]
pub struct BinaryName<'a>(&'a str);

impl<'a> BinaryName<'a> {
    /// Creates a `BinaryName` from the given string slice.
    #[must_use]
    pub const fn new(name: &'a str) -> Self { Self(name) }

    /// Returns the binary name as a string slice.
    #[must_use]
    pub const fn as_str(self) -> &'a str { self.0 }
}

impl<'a> From<&'a str> for BinaryName<'a> {
    fn from(s: &'a str) -> Self { Self(s) }
}

impl std::fmt::Display for BinaryName<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(self.0) }
}

/// Captured output from `cargo build --bin` when it exits unsuccessfully.
#[derive(Debug)]
pub struct BuildFailureCapture {
    /// Exit status reported by `cargo build --bin`.
    pub status: ExitStatus,
    /// Bytes captured from the command's standard output.
    pub stdout: Vec<u8>,
    /// Bytes captured from the command's standard error.
    pub stderr: Vec<u8>,
}

impl fmt::Display for BuildFailureCapture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let stdout = String::from_utf8_lossy(&self.stdout);
        let stderr = String::from_utf8_lossy(&self.stderr);
        write!(
            f,
            "`cargo build --bin` failed with status {}\nstdout:\n{stdout}\nstderr:\n{stderr}",
            self.status,
        )
    }
}

impl std::error::Error for BuildFailureCapture {}

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
    #[error(transparent)]
    BuildFailed(#[from] BuildFailureCapture),

    /// The expected binary path exists neither on disk nor after a build.
    #[error("could not locate or build workspace binary at {}", .0.display())]
    MissingBinary(PathBuf),
}

// Adapted from assert_cmd's cargo helper: same `CARGO_BIN_EXE_<name>` convention
// and `current_exe`-derived fallback.
fn cargo_bin_path_for_integration_tests(binary_name: BinaryName<'_>) -> PathBuf {
    let env_var = format!("CARGO_BIN_EXE_{binary_name}");
    env::var_os(&env_var).map_or_else(
        || {
            env::current_exe().ok().map_or_else(
                || fallback_binary_path(binary_name, None),
                |executable| fallback_binary_path(binary_name, Some(&executable)),
            )
        },
        PathBuf::from,
    )
}

/// Where `<binary_name>` sits relative to the executable running this test.
///
/// Integration tests are linked into `<profile>/deps` and run from there, so
/// the binary a sibling crate produced is `<profile>/<name>`, not
/// `<profile>/deps/<name>` and not `<target>/<name>`. Dropping this path
/// silently moved the search up two levels: the fallback then looked for
/// `<target>/<name>`, missed, and fell through to `cargo metadata`, which
/// reports the *default* target directory and can hand back a binary from a
/// different one. The tag lookup in [`target_directory_of`] answers "where is
/// the target root"; the profile is the parent of the directory the test
/// binary sits in, which is a separate question.
///
/// The layout question is [`profile_dir_of_test_executable`]'s, so this
/// function only chooses the default for an absent executable. Taking the
/// executable as a parameter rather than reading `current_exe` here is what
/// lets a test drive the whole fallback, including the `.` default, without
/// spawning a binary.
fn fallback_binary_path(binary_name: BinaryName<'_>, executable: Option<&Path>) -> PathBuf {
    let profile = executable.map_or_else(|| PathBuf::from("."), profile_dir_of_test_executable);
    profile.join(format!("{binary_name}{}", env::consts::EXE_SUFFIX))
}

/// Cargo's cache-directory tag, written at the root of every target directory.
///
/// Cargo emits this once per target directory and nowhere below it, so its
/// presence identifies the root without relying on any file's placement. That
/// matters because Cargo's build-directory layout is explicitly not a stable
/// interface and changed under these helpers once already: 1.100.0 moved every
/// compiled unit out of `<profile>/deps` into `<profile>/build/<pkg>/<hash>/out`,
/// so `deps` is no longer a directory that can be named.
const CACHEDIR_TAG: &str = "CACHEDIR.TAG";

/// The target root that `executable` was built beneath.
///
/// Walks up from the executable until it finds the directory holding Cargo's
/// `CACHEDIR.TAG`. That is the same answer for `<target>/debug/deps/x` under
/// the classic layout and for `<target>/debug/build/<pkg>/<hash>/out/x` under
/// the new one, and it also honours a `--target-dir` override such as the
/// subdirectory `cargo llvm-cov` selects, which `cargo metadata` cannot see.
///
/// Returning `None` means no ancestor carried the tag, so the caller must fall
/// back to metadata rather than guess a directory.
#[must_use]
pub fn target_directory_of(executable: &Path) -> Option<PathBuf> {
    executable
        .ancestors()
        .skip(1)
        .find(|ancestor| ancestor.join(CACHEDIR_TAG).is_file())
        .map(Path::to_path_buf)
}

/// Locate the profile directory that holds workspace binaries, given a test
/// executable's path. Pure computation: no I/O. An executable outside Cargo's
/// `deps` and `build/<package>/<hash>/out` layouts resolves to its own parent
/// directory; the caller's `is_file` probe then finds a workspace binary only if
/// one happens to sit beside the test executable, and otherwise misses.
fn profile_dir_of_test_executable(executable: &Path) -> PathBuf {
    let mut path = executable.to_path_buf();
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    } else if path.ends_with("out") {
        // Cargo's per-package build-directory layout places test executables
        // at `<profile>/build/<package>/<hash>/out/<exe>`; pop `out`, the
        // hash, the package, and `build` to reach the profile directory.
        let layout_is_build_directory = path
            .ancestors()
            .nth(3)
            .is_some_and(|build| build.ends_with("build"));
        if layout_is_build_directory {
            for _ in 0..4 {
                path.pop();
            }
        }
    }
    path
}

fn try_command_from_cargo_test_bin_layout(binary_name: BinaryName<'_>) -> Option<Command> {
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
///
/// use rstest_bdd_harness::binary_test_support::{BinaryName, binary_path_in_target_dir};
///
/// let path = binary_path_in_target_dir(Path::new("/tmp/ws/target"), BinaryName::new("my-bin"));
/// let suffix = std::env::consts::EXE_SUFFIX;
/// assert_eq!(
///     path,
///     Path::new("/tmp/ws/target/debug").join(format!("my-bin{suffix}"))
/// );
/// ```
#[must_use]
pub fn binary_path_in_target_dir(target_directory: &Path, binary_name: BinaryName<'_>) -> PathBuf {
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
///
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
///
/// use rstest_bdd_harness::binary_test_support::{BinaryName, locate_or_build_binary};
///
/// let cmd = locate_or_build_binary(
///     Path::new("Cargo.toml"),
///     Path::new("."),
///     BinaryName::new("my-bin"),
/// )
/// .expect("locate binary");
/// let _ = cmd;
/// ```
///
/// Strategy:
/// 1. Try the path implied by `CARGO_BIN_EXE_<binary_name>` (and the same `current_exe` fallback
///    used by `assert_cmd`'s `cargo_bin` helper).
/// 2. On failure, resolve the expected debug binary path via `target_directory_for_manifest`.
/// 3. If the binary is absent, invoke `build_binary` and surface stdout/stderr on failure.
/// 4. Return `Command::new(binary)` when the binary is present.
pub fn locate_or_build_binary(
    manifest_path: &Path,
    workspace_root: &Path,
    binary_name: BinaryName<'_>,
) -> Result<Command, BinaryLocateError> {
    if let Some(command) = try_command_from_cargo_test_bin_layout(binary_name) {
        return Ok(command);
    }
    let target_dir = target_directory_for_manifest(manifest_path)?;
    let binary = binary_path_in_target_dir(&target_dir, binary_name);
    if !binary.is_file() {
        let output = build_binary(workspace_root, binary_name)?;
        if !output.status.success() {
            return Err(BuildFailureCapture {
                status: output.status,
                stdout: output.stdout,
                stderr: output.stderr,
            }
            .into());
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
///
/// use rstest_bdd_harness::binary_test_support::{BinaryName, build_binary};
///
/// let output = build_binary(Path::new("."), BinaryName::new("some-bin")).expect("spawn cargo");
/// assert!(output.status.success() || !output.stderr.is_empty());
/// ```
pub fn build_binary(workspace_root: &Path, binary_name: BinaryName<'_>) -> std::io::Result<Output> {
    let cargo = option_env!("CARGO").unwrap_or("cargo");
    Command::new(cargo)
        .current_dir(workspace_root)
        .args(["build", "--bin", binary_name.as_str()])
        .output()
}

#[cfg(test)]
mod tests {
    //! Fast unit tests that avoid spawning `cargo`; subprocess coverage lives in
    //! `tests/binary_test_support_cargo.rs` (non-Windows only).

    use std::path::Path;

    use super::{
        BinaryName,
        binary_path_in_target_dir,
        fallback_binary_path,
        profile_dir_of_test_executable,
    };

    // ── profile_dir_of_test_executable ─────────────────────────────────────

    #[rstest::rstest]
    #[case::classic_layout("/t/debug/deps/it-abc", "/t/debug")]
    #[case::build_directory_layout("/t/debug/build/pkg/abc/out/it-abc", "/t/debug")]
    #[case::classic_layout_coverage_target(
        "/t/llvm-cov-target/debug/deps/it-abc",
        "/t/llvm-cov-target/debug"
    )]
    #[case::build_directory_layout_coverage_target(
        "/t/llvm-cov-target/debug/build/pkg/abc/out/it-abc",
        "/t/llvm-cov-target/debug"
    )]
    #[case::unrecognized_directory("/t/debug/examples/it-abc", "/t/debug/examples")]
    #[case::out_outside_build_directory("/t/x/pkg/abc/out/it-abc", "/t/x/pkg/abc/out")]
    fn profile_directory_follows_the_cargo_layout(
        #[case] executable: &str,
        #[case] expected: &str,
    ) {
        assert_eq!(
            profile_dir_of_test_executable(Path::new(executable)),
            Path::new(expected),
            "{executable} should resolve to {expected}"
        );
    }

    // ── binary_path_in_target_dir ──────────────────────────────────────────

    #[test]
    fn binary_path_appends_debug_and_exe_suffix() {
        let target = Path::new("/workspace/target");
        let path = binary_path_in_target_dir(target, BinaryName::new("my-tool"));
        let expected_name = format!("my-tool{}", std::env::consts::EXE_SUFFIX);
        assert_eq!(path, target.join("debug").join(expected_name));
    }

    #[test]
    fn binary_path_uses_provided_target_directory() {
        let target = Path::new("/custom/target/dir");
        let path = binary_path_in_target_dir(target, BinaryName::new("tool"));
        assert!(path.starts_with(target));
    }

    #[test]
    fn the_binary_fallback_searches_the_profile_not_the_target_root() {
        // The regression this guards: the fallback used to derive the target
        // *root* from `CACHEDIR.TAG` and join the binary name straight onto it,
        // producing `<target>/tool` where the binary is `<target>/debug/tool`.
        // It then missed and fell through to `cargo metadata`, whose answer is
        // the default target directory and can differ from the one this build
        // used. Driving the whole fallback is what makes this case fail if it
        // reverts to the root or loses the `deps` stripping; asserting on the
        // derivation alone would pass for both.
        let name = BinaryName::new("tool");
        let suffix = std::env::consts::EXE_SUFFIX;
        assert_eq!(
            fallback_binary_path(name, Some(Path::new("/cache/debug/deps/test"))),
            Path::new("/cache/debug").join(format!("tool{suffix}"))
        );
        // A binary that is not under `deps` — a plain `cargo run` target —
        // is anchored on the profile directly.
        assert_eq!(
            fallback_binary_path(name, Some(Path::new("/cache/debug/tool"))),
            Path::new("/cache/debug").join(format!("tool{suffix}"))
        );
        // With no executable to inspect there is nothing to anchor on, and the
        // relative fallback must stay relative rather than guessing a root.
        assert_eq!(
            fallback_binary_path(name, None),
            Path::new(".").join(format!("tool{suffix}"))
        );
    }
}
