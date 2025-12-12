//! Crate-root discovery using cargo metadata.
//!
//! This module provides functionality for discovering workspace information
//! from a given path using `cargo metadata`. It identifies the workspace root,
//! package names, and feature file locations.

use std::path::{Path, PathBuf};

use cargo_metadata::MetadataCommand;

use crate::error::ServerError;

/// Information about a discovered workspace.
///
/// Contains the workspace root path and the names of packages within the
/// workspace.
#[derive(Debug, Clone)]
pub struct WorkspaceInfo {
    /// Path to the workspace root (directory containing the root Cargo.toml).
    pub root: PathBuf,
    /// Package names within the workspace.
    pub packages: Vec<String>,
}

/// Discover workspace information from a given path.
///
/// Uses `cargo metadata` to find the workspace root and enumerate packages.
/// The path can be any file or directory within the workspace.
///
/// # Arguments
///
/// * `path` - A path within the workspace (file or directory)
///
/// # Errors
///
/// Returns `ServerError::CargoMetadata` if the metadata command fails, or
/// `ServerError::WorkspaceDiscovery` if the path is not within a Cargo
/// workspace.
///
/// # Examples
///
/// ```ignore
/// use std::path::Path;
/// use rstest_bdd_server::discovery::discover_workspace;
///
/// let info = discover_workspace(Path::new("/path/to/project"))?;
/// println!("Workspace root: {}", info.root.display());
/// ```
pub fn discover_workspace(path: &Path) -> Result<WorkspaceInfo, ServerError> {
    let manifest_path = find_manifest_path(path)?;

    let metadata = MetadataCommand::new()
        .manifest_path(&manifest_path)
        .no_deps()
        .exec()?;

    let packages = metadata.packages.iter().map(|p| p.name.clone()).collect();

    Ok(WorkspaceInfo {
        root: metadata.workspace_root.into_std_path_buf(),
        packages,
    })
}

/// Find the nearest Cargo.toml manifest file from a given path.
///
/// Walks up the directory tree from the given path until a Cargo.toml is found.
///
/// # Errors
///
/// Returns `ServerError::WorkspaceDiscovery` if no Cargo.toml is found.
fn find_manifest_path(path: &Path) -> Result<PathBuf, ServerError> {
    let start = if path.is_file() {
        path.parent().unwrap_or(path)
    } else {
        path
    };

    let mut current = start;
    loop {
        let manifest = current.join("Cargo.toml");
        if manifest.exists() {
            return Ok(manifest);
        }
        match current.parent() {
            Some(parent) => current = parent,
            None => {
                return Err(ServerError::WorkspaceDiscovery(format!(
                    "no Cargo.toml found in {} or any parent directory",
                    start.display()
                )))
            }
        }
    }
}

/// Find all `.feature` files within a workspace.
///
/// Searches common locations for Gherkin feature files:
/// - `tests/features/`
/// - `features/`
/// - Any subdirectory matching `**/features/*.feature`
///
/// # Arguments
///
/// * `workspace_root` - The root directory of the workspace
///
/// # Examples
///
/// ```ignore
/// use std::path::Path;
/// use rstest_bdd_server::discovery::find_feature_files;
///
/// let features = find_feature_files(Path::new("/path/to/project"));
/// for path in features {
///     println!("Found feature: {}", path.display());
/// }
/// ```
#[must_use]
pub fn find_feature_files(workspace_root: &Path) -> Vec<PathBuf> {
    let mut features = Vec::new();

    // Check common feature file locations
    let search_dirs = [
        workspace_root.join("tests").join("features"),
        workspace_root.join("features"),
    ];

    for dir in &search_dirs {
        if dir.is_dir() {
            collect_feature_files_recursive(dir, &mut features);
        }
    }

    // Also search in crate subdirectories
    search_crate_subdirectories(workspace_root, &mut features);

    features
}

/// Search for feature files in crate subdirectories.
///
/// Looks for `tests/features/` directories within each subdirectory of the
/// workspace root (typical layout for multi-crate workspaces).
fn search_crate_subdirectories(workspace_root: &Path, features: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(workspace_root) else {
        return;
    };

    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let crate_features = path.join("tests").join("features");
        if crate_features.is_dir() {
            collect_feature_files_recursive(&crate_features, features);
        }
    }
}

/// Recursively collect `.feature` files from a directory.
fn collect_feature_files_recursive(dir: &Path, features: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                collect_feature_files_recursive(&path, features);
            } else if path.extension().is_some_and(|ext| ext == "feature") {
                features.push(path);
            }
        }
    }
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "tests require explicit panic messages for debugging failures"
)]
mod tests {
    use super::*;
    use rstest::rstest;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_workspace() -> TempDir {
        let dir = TempDir::new().expect("failed to create temp dir");
        let cargo_toml = dir.path().join("Cargo.toml");
        fs::write(
            &cargo_toml,
            r#"[package]
name = "test-project"
version = "0.1.0"
edition = "2021"
"#,
        )
        .expect("failed to write Cargo.toml");

        // Create a simple src/lib.rs so the package is valid
        let src_dir = dir.path().join("src");
        fs::create_dir_all(&src_dir).expect("failed to create src dir");
        fs::write(src_dir.join("lib.rs"), "").expect("failed to write lib.rs");

        dir
    }

    #[rstest]
    fn discovers_workspace_from_root() {
        let workspace = create_test_workspace();
        let result = discover_workspace(workspace.path());
        assert!(result.is_ok());
        let info = result.expect("should discover workspace");
        assert_eq!(info.root, workspace.path());
        assert!(info.packages.contains(&"test-project".to_string()));
    }

    #[rstest]
    fn discovers_workspace_from_subdirectory() {
        let workspace = create_test_workspace();
        let subdir = workspace.path().join("src");
        let result = discover_workspace(&subdir);
        assert!(result.is_ok());
        let info = result.expect("should discover workspace");
        assert_eq!(info.root, workspace.path());
    }

    #[rstest]
    fn fails_when_no_manifest_found() {
        let dir = TempDir::new().expect("failed to create temp dir");
        let result = discover_workspace(dir.path());
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("no Cargo.toml found"));
    }

    /// Creates a test workspace with a feature file in a specified directory.
    ///
    /// # Arguments
    ///
    /// * `relative_dir` - Path segments relative to the workspace root (e.g., `&["tests", "features"]`)
    /// * `filename` - Name of the feature file to create
    /// * `content` - Content to write to the feature file
    ///
    /// # Returns
    ///
    /// A tuple of `(TempDir, Vec<PathBuf>)` containing the workspace directory
    /// and the result of calling `find_feature_files` on it.
    fn create_workspace_with_feature(
        relative_dir: &[&str],
        filename: &str,
        content: &str,
    ) -> (TempDir, Vec<PathBuf>) {
        let workspace = create_test_workspace();
        let mut dir = workspace.path().to_path_buf();
        for segment in relative_dir {
            dir = dir.join(segment);
        }
        fs::create_dir_all(&dir).expect("failed to create feature dir");
        fs::write(dir.join(filename), content).expect("failed to write feature file");

        let features = find_feature_files(workspace.path());
        (workspace, features)
    }

    #[rstest]
    #[case(&["tests", "features"], "example.feature", "Feature: Test")]
    #[case(&["tests", "features", "nested"], "nested.feature", "Feature: Nested")]
    fn finds_feature_files_in_various_locations(
        #[case] relative_dir: &[&str],
        #[case] filename: &str,
        #[case] content: &str,
    ) {
        let (_workspace, features) = create_workspace_with_feature(relative_dir, filename, content);

        assert_eq!(features.len(), 1);
        assert!(features
            .first()
            .expect("should have one feature")
            .ends_with(filename));
    }

    #[rstest]
    fn returns_empty_when_no_feature_files() {
        let workspace = create_test_workspace();
        let features = find_feature_files(workspace.path());
        assert!(features.is_empty());
    }
}
