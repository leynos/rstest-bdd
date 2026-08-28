//! Compile-fail fixture for the former infallible workspace discovery contract.

use std::path::{Path, PathBuf};

use rstest_bdd_server::discovery::find_feature_files;

/// Attempts to use the former direct `Vec<PathBuf>` return contract.
fn discover_features(workspace_root: &Path) -> Vec<PathBuf> {
    find_feature_files(workspace_root)
}

/// Provides the binary entry point required by the compile-fail fixture.
fn main() {
    let _ = discover_features(Path::new("."));
}
