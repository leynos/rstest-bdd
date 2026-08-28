//! Deterministic failure coverage for workspace directory traversal.

use std::io;

use super::{tests::assert_io_error_kind, *};

/// Selects the deterministic filesystem operation that returns an I/O error.
#[derive(Clone, Copy)]
enum ReaderFailure {
    /// Fails directory metadata checks for optional feature directories.
    Metadata,
    /// Fails while iterating a nested feature directory.
    NestedDirectoryEntry,
    /// Fails the crate-manifest metadata probe.
    ManifestMetadata,
    /// Fails while iterating the workspace root.
    WorkspaceDirectoryEntry,
}

/// Supplies deterministic directory-operation failures for traversal tests.
struct FailingDirectoryReader {
    failure: ReaderFailure,
}

impl DirectoryReader for FailingDirectoryReader {
    type Entries = std::vec::IntoIter<io::Result<PathBuf>>;

    /// Returns configured directory-entry failures at the selected traversal point.
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

    /// Returns configured directory metadata errors or deterministic directory states.
    fn is_directory(&self, path: &Path) -> io::Result<bool> {
        match self.failure {
            ReaderFailure::Metadata => Err(io::Error::from(io::ErrorKind::PermissionDenied)),
            ReaderFailure::ManifestMetadata => Ok(!path.ends_with("features")),
            ReaderFailure::WorkspaceDirectoryEntry => Ok(false),
            ReaderFailure::NestedDirectoryEntry => Ok(true),
        }
    }

    /// Returns the configured crate-manifest metadata error or a non-file state.
    fn is_file(&self, _path: &Path) -> io::Result<bool> {
        match self.failure {
            ReaderFailure::ManifestMetadata => {
                Err(io::Error::from(io::ErrorKind::PermissionDenied))
            }
            _ => Ok(false),
        }
    }
}

/// Returns `ServerError::Io` with `io::ErrorKind::PermissionDenied` for feature metadata failure.
#[test]
fn reports_optional_feature_directory_metadata_failure() {
    let reader = FailingDirectoryReader {
        failure: ReaderFailure::Metadata,
    };

    let error = find_feature_files_with(Path::new("workspace"), &reader)
        .expect_err("unreadable feature directory should return an I/O error");

    assert_io_error_kind(error, io::ErrorKind::PermissionDenied);
}

/// Returns `ServerError::Io` with `io::ErrorKind::PermissionDenied` for a nested entry failure.
#[test]
fn reports_nested_feature_directory_entry_failure() {
    let reader = FailingDirectoryReader {
        failure: ReaderFailure::NestedDirectoryEntry,
    };

    let error = find_feature_files_with(Path::new("workspace"), &reader)
        .expect_err("unreadable nested feature directory should return an I/O error");

    assert_io_error_kind(error, io::ErrorKind::PermissionDenied);
}

/// Returns `ServerError::Io` with `io::ErrorKind::PermissionDenied` for crate manifest metadata.
#[test]
fn reports_crate_manifest_metadata_failure() {
    let reader = FailingDirectoryReader {
        failure: ReaderFailure::ManifestMetadata,
    };

    let error = find_feature_files_with(Path::new("workspace"), &reader)
        .expect_err("unreadable crate manifest should return an I/O error");

    assert_io_error_kind(error, io::ErrorKind::PermissionDenied);
}

/// Returns `ServerError::Io` with `io::ErrorKind::PermissionDenied` for a workspace entry failure.
#[test]
fn reports_workspace_directory_entry_failure() {
    let reader = FailingDirectoryReader {
        failure: ReaderFailure::WorkspaceDirectoryEntry,
    };

    let error = find_feature_files_with(Path::new("workspace"), &reader)
        .expect_err("unreadable workspace directory entry should return an I/O error");

    assert_io_error_kind(error, io::ErrorKind::PermissionDenied);
}

/// Returns `ServerError::Io` with `io::ErrorKind::PermissionDenied` for recursive entry failure.
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
