//! Shared helpers for harness crates that run `macrotest` against snapshot
//! fixtures.
//!
//! These utilities back the `macro_compile` integration suites in the Tokio
//! and GPUI harness crates so both can resolve `cargo expand`, decide whether
//! macrotest refresh is enabled, and perform substring assertions over
//! `.expanded.rs` snapshot files without re-implementing identical code in
//! each crate. The module is exposed as a hidden API for those harness tests
//! only and is not part of the supported public surface.
//!
//! Snapshot paths are passed as absolute paths because each caller's
//! `CARGO_MANIFEST_DIR` resolves at compile time inside the caller crate;
//! sharing the helpers via this module avoids that per-crate coupling.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Returns `true` when the macrotest refresh workflow is enabled.
///
/// The workflow requires the `RSTEST_BDD_RUN_MACROTEST` environment variable
/// to be set and the `cargo expand` subcommand to be available. Both
/// conditions must hold for `macrotest::expand_without_refresh` to produce a
/// useful comparison against the checked-in `.expanded.rs` files.
#[must_use]
pub fn snapshot_refresh_is_enabled() -> bool {
    std::env::var_os("RSTEST_BDD_RUN_MACROTEST").is_some() && cargo_expand_is_available()
}

fn cargo_expand_is_available() -> bool {
    Command::new("cargo")
        .args(["expand", "--version"])
        .output()
        .is_ok_and(|output| output.status.success())
}

/// Reads a snapshot file at `path`, panicking with file context on failure.
fn read_snapshot(path: &Path) -> String {
    match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(err) => panic!("failed to read snapshot {}: {err}", path.display()),
    }
}

/// Asserts that every needle appears at least once in the snapshot at `path`.
///
/// # Panics
///
/// Panics if `path` cannot be read (I/O error), or if any needle is absent
/// from the snapshot contents.
pub fn assert_snapshot_contains(path: &Path, needles: &[&str]) {
    let contents = read_snapshot(path);
    for needle in needles {
        assert!(
            contents.contains(needle),
            "expected {} to contain {needle:?}",
            path.display(),
        );
    }
}

/// Asserts that `needle` does not appear anywhere in the snapshot at `path`.
///
/// # Panics
///
/// Panics if `path` cannot be read (I/O error), or if `needle` is found in
/// the snapshot contents.
pub fn assert_snapshot_omits(path: &Path, needle: &str) {
    let contents = read_snapshot(path);
    assert!(
        !contents.contains(needle),
        "expected {} to omit {needle:?}",
        path.display(),
    );
}

/// Computes the trybuild scratch directory for the crate whose manifest lives
/// at `manifest_path`.
///
/// The returned path joins the workspace `target` directory with
/// `tests/trybuild/<target_subdir>`, matching the layout trybuild uses for
/// per-crate scratch space.
pub fn trybuild_crate_root(
    manifest_path: &Path,
    target_subdir: &str,
) -> Result<PathBuf, Box<dyn Error>> {
    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path(manifest_path)
        .no_deps()
        .exec()?;
    Ok(metadata
        .target_directory
        .into_std_path_buf()
        .join("tests/trybuild")
        .join(target_subdir))
}
