//! Behavioural tests for the public workspace feature-discovery API.

use std::{io, path::PathBuf};

use cap_std::{ambient_authority, fs::Dir};
use rstest_bdd_server::{discovery::find_feature_files, error::ServerError};
use tempfile::TempDir;

/// Creates and removes a temporary directory, returning its missing path.
fn create_missing_path() -> io::Result<PathBuf> {
    let temporary_directory = TempDir::new()?;
    let missing_path = temporary_directory.path().to_path_buf();
    temporary_directory.close()?;

    Ok(missing_path)
}

/// Discovers a nested feature file through the exported API.
#[test]
fn discovers_nested_feature_file() {
    let workspace = TempDir::new().expect("test setup should create a workspace");
    let features_directory = workspace.path().join("tests/features/nested");
    let feature_path = features_directory.join("public-api.feature");
    let workspace_directory = Dir::open_ambient_dir(workspace.path(), ambient_authority())
        .expect("test setup should open a workspace capability");
    workspace_directory
        .create_dir_all("tests/features/nested")
        .expect("test setup should create feature directories");
    workspace_directory
        .write(
            "tests/features/nested/public-api.feature",
            "Feature: Public API discovery",
        )
        .expect("test setup should write a feature file");

    let features = find_feature_files(workspace.path())
        .expect("public feature discovery should succeed for a readable workspace");

    assert!(
        features.contains(&feature_path),
        "public feature discovery should return the nested feature path"
    );
}

/// Returns `ServerError::Io` with `io::ErrorKind::NotFound` for a missing workspace.
#[test]
fn reports_missing_workspace_error() {
    let missing_path =
        create_missing_path().expect("test setup should remove a temporary directory");

    let error = find_feature_files(&missing_path)
        .expect_err("public feature discovery should fail for a missing workspace");

    let ServerError::Io(error) = error else {
        panic!("missing workspace should return an I/O error");
    };
    assert_eq!(error.kind(), io::ErrorKind::NotFound);
}
