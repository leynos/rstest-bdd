//! Compile-pass fixture for fallible public workspace feature discovery.

use std::path::{Path, PathBuf};

use rstest_bdd_server::{
    discovery::find_feature_files,
    error::ServerError,
};

/// Handles the public discovery result with error propagation.
fn discover_features(workspace_root: &Path) -> Result<Vec<PathBuf>, ServerError> {
    let features = find_feature_files(workspace_root)?;
    Ok(features)
}

/// Provides the binary entry point required by the compile-pass fixture.
fn main() {
    let _ = discover_features(Path::new("."));
}
