//! Snapshot and refresh helpers for the `macrotest` harness suites.
//!
//! Deciding whether refresh is enabled and asserting substrings over an
//! `.expanded.rs` snapshot, kept apart from the trybuild tree management in
//! [`macrotest_support`](super) because the two do not depend on each other:
//! these read a checked-in file, while those build and locate a tree.

use std::{fs, path::Path, process::Command};

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
    let missing = needles
        .iter()
        .copied()
        .filter(|needle| !contents.contains(needle))
        .collect::<Vec<_>>();
    assert!(
        missing.is_empty(),
        "expected {} to contain {missing:?}",
        path.display(),
    );
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
