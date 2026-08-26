//! Unit tests for workspace discovery.

use std::{fs, io};

use rstest::{fixture, rstest};
use tempfile::TempDir;

use super::*;

#[rstest_bdd_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn create_test_workspace() -> io::Result<TempDir> {
    let dir = TempDir::new()?;
    let cargo_toml = dir.path().join("Cargo.toml");
    fs::write(
        &cargo_toml,
        r#"[package]
name = "test-project"
version = "0.1.0"
edition = "2024"
"#,
    )?;

    // Create a simple src/lib.rs so the package is valid
    let src_dir = dir.path().join("src");
    fs::create_dir_all(&src_dir)?;
    fs::write(src_dir.join("lib.rs"), "")?;

    Ok(dir)
}

#[rstest]
fn discovers_workspace_from_root(create_test_workspace: io::Result<TempDir>) {
    let workspace = create_test_workspace.expect("test setup should succeed");
    let result = discover_workspace(workspace.path());
    assert!(result.is_ok());
    let info = result.expect("should discover workspace");
    assert_eq!(info.root, workspace.path());
    assert!(info.packages.contains(&"test-project".to_owned()));
}

#[rstest]
fn discovers_workspace_from_subdirectory(create_test_workspace: io::Result<TempDir>) {
    let workspace = create_test_workspace.expect("test setup should succeed");
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
    let err = result.expect_err("workspace discovery should fail when no Cargo.toml exists");
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
/// The result of calling `find_feature_files` on the workspace.
fn create_workspace_with_feature(
    relative_dir: &[&str],
    filename: &str,
    content: &str,
) -> Result<Vec<PathBuf>, ServerError> {
    let workspace = create_test_workspace()?;
    let mut dir = workspace.path().to_path_buf();
    for segment in relative_dir {
        dir = dir.join(segment);
    }
    fs::create_dir_all(&dir)?;
    fs::write(dir.join(filename), content)?;

    find_feature_files(workspace.path())
}

#[rstest]
#[case(&["tests", "features"], "example.feature", "Feature: Test")]
#[case(&["tests", "features", "nested"], "nested.feature", "Feature: Nested")]
fn finds_feature_files_in_various_locations(
    #[case] relative_dir: &[&str],
    #[case] filename: &str,
    #[case] content: &str,
) {
    let features = create_workspace_with_feature(relative_dir, filename, content)
        .expect("test setup and feature discovery should succeed");

    assert_eq!(features.len(), 1);
    assert!(
        features
            .first()
            .expect("should have one feature")
            .ends_with(filename)
    );
}

#[rstest]
fn returns_empty_when_no_feature_files(create_test_workspace: io::Result<TempDir>) {
    let workspace = create_test_workspace.expect("test setup should succeed");
    let features = find_feature_files(workspace.path()).expect("feature discovery should succeed");
    assert!(features.is_empty());
}

fn create_missing_path() -> PathBuf {
    let temporary_directory = match TempDir::new() {
        Ok(temporary_directory) => temporary_directory,
        Err(error) => panic!("failed to create temp dir: {error}"),
    };
    let missing_path = temporary_directory.path().to_path_buf();
    if let Err(error) = temporary_directory.close() {
        panic!("failed to remove temp dir: {error}");
    }

    missing_path
}

fn assert_not_found_error(error: ServerError) {
    assert_io_error_kind(error, io::ErrorKind::NotFound);
}

fn assert_io_error_kind(error: ServerError, expected_kind: io::ErrorKind) {
    let ServerError::Io(source) = error else {
        panic!("expected an I/O error");
    };

    assert_eq!(source.kind(), expected_kind);
}

#[test]
fn reports_workspace_read_failure() {
    let missing_path = create_missing_path();

    let error = find_feature_files(&missing_path)
        .expect_err("missing workspace should return an I/O error");

    assert_not_found_error(error);
}

#[test]
fn reports_recursive_directory_read_failure() {
    let missing_path = create_missing_path();
    let mut features = Vec::new();

    let error =
        collect_feature_files_recursive(&StandardDirectoryReader, &missing_path, &mut features)
            .expect_err("missing feature directory should return an I/O error");

    assert_not_found_error(error);
}

#[derive(Clone, Copy)]
enum ReaderFailure {
    Metadata,
    NestedDirectoryEntry,
    ManifestMetadata,
    WorkspaceDirectoryEntry,
}

struct FailingDirectoryReader {
    failure: ReaderFailure,
}

impl DirectoryReader for FailingDirectoryReader {
    type Entries = std::vec::IntoIter<io::Result<PathBuf>>;

    fn read_dir(&self, path: &Path) -> io::Result<Self::Entries> {
        let entries = match self.failure {
            ReaderFailure::NestedDirectoryEntry if path.ends_with("nested") => {
                vec![Err(io::Error::from(io::ErrorKind::PermissionDenied))]
            }
            ReaderFailure::ManifestMetadata if path == Path::new("workspace") => {
                vec![Ok(path.join("crate"))]
            }
            ReaderFailure::WorkspaceDirectoryEntry if path == Path::new("workspace") => {
                vec![Err(io::Error::from(io::ErrorKind::PermissionDenied))]
            }
            _ => vec![Ok(path.join("nested"))],
        };

        Ok(entries.into_iter())
    }

    fn is_directory(&self, path: &Path) -> io::Result<bool> {
        match self.failure {
            ReaderFailure::Metadata => Err(io::Error::from(io::ErrorKind::PermissionDenied)),
            ReaderFailure::ManifestMetadata => Ok(!path.ends_with("features")),
            ReaderFailure::WorkspaceDirectoryEntry => Ok(false),
            ReaderFailure::NestedDirectoryEntry => Ok(true),
        }
    }

    fn is_file(&self, _path: &Path) -> io::Result<bool> {
        match self.failure {
            ReaderFailure::ManifestMetadata => {
                Err(io::Error::from(io::ErrorKind::PermissionDenied))
            }
            _ => Ok(false),
        }
    }
}

#[test]
fn reports_optional_feature_directory_metadata_failure() {
    let reader = FailingDirectoryReader {
        failure: ReaderFailure::Metadata,
    };

    let error = find_feature_files_with(Path::new("workspace"), &reader)
        .expect_err("unreadable feature directory should return an I/O error");

    assert_io_error_kind(error, io::ErrorKind::PermissionDenied);
}

#[test]
fn reports_nested_feature_directory_entry_failure() {
    let reader = FailingDirectoryReader {
        failure: ReaderFailure::NestedDirectoryEntry,
    };

    let error = find_feature_files_with(Path::new("workspace"), &reader)
        .expect_err("unreadable nested feature directory should return an I/O error");

    assert_io_error_kind(error, io::ErrorKind::PermissionDenied);
}

#[test]
fn reports_crate_manifest_metadata_failure() {
    let reader = FailingDirectoryReader {
        failure: ReaderFailure::ManifestMetadata,
    };

    let error = find_feature_files_with(Path::new("workspace"), &reader)
        .expect_err("unreadable crate manifest should return an I/O error");

    assert_io_error_kind(error, io::ErrorKind::PermissionDenied);
}

#[test]
fn reports_workspace_directory_entry_failure() {
    let reader = FailingDirectoryReader {
        failure: ReaderFailure::WorkspaceDirectoryEntry,
    };

    let error = find_feature_files_with(Path::new("workspace"), &reader)
        .expect_err("unreadable workspace directory entry should return an I/O error");

    assert_io_error_kind(error, io::ErrorKind::PermissionDenied);
}
#[test]
fn reports_recursive_directory_entry_failure() {
    let reader = FailingDirectoryReader {
        failure: ReaderFailure::NestedDirectoryEntry,
    };
    let mut features = Vec::new();

    let error =
        collect_feature_files_recursive(&reader, Path::new("feature-directory"), &mut features)
            .expect_err("unreadable nested feature directory should return an I/O error");

    assert_io_error_kind(error, io::ErrorKind::PermissionDenied);
}
