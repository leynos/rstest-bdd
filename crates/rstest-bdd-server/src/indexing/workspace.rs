//! Capability-scoped workspace-root access for feature indexing.
//!
//! This module owns the validated root capability shared by `ServerState` and
//! disk-backed feature indexing. The server retains the capability, while the
//! indexer receives already-read source text. Workspace-relative
//! path validation and capability-rooted file reads live here at the server
//! boundary rather than in the indexing domain.

use std::fmt;
use std::path::{Component, Path};

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;

use crate::error::ServerError;
use crate::indexing::FeatureIndexError;

/// Validated capability for reading files beneath one workspace root.
pub struct WorkspaceRoot {
    path: Utf8PathBuf,
    directory: Dir,
}

impl fmt::Debug for WorkspaceRoot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WorkspaceRoot")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl WorkspaceRoot {
    /// Open a capability-scoped directory rooted at `path`.
    ///
    /// # Errors
    ///
    /// Returns a [`ServerError`] when the path is not UTF-8 or the directory
    /// cannot be opened as an ambient capability. Opening is blocking and must
    /// run off the LSP executor.
    pub fn open(path: &Path) -> Result<Self, ServerError> {
        let path = Utf8Path::from_path(path)
            .ok_or_else(|| ServerError::WorkspaceRootNotUtf8(path.to_path_buf()))?
            .to_path_buf();
        let directory = Dir::open_ambient_dir(&path, ambient_authority())?;

        Ok(Self { path, directory })
    }

    /// Return the validated root path.
    #[must_use]
    pub fn path(&self) -> &Utf8Path {
        &self.path
    }

    /// Read a feature file beneath this workspace root through the retained
    /// capability.
    ///
    /// The supplied path must resolve to a file beneath the workspace root:
    /// absolute paths outside the root, `..` traversal, and non-UTF-8 relative
    /// paths are rejected at this boundary before any filesystem access. On
    /// success the source text is returned without a trailing newline
    /// guarantee; callers feed it to `index_feature_source`, which applies the
    /// canonical trailing-newline normalization.
    ///
    /// # Errors
    ///
    /// Returns a [`FeatureIndexError`] when the path is outside the workspace
    /// root, is not valid UTF-8 relative to it, or cannot be read.
    pub(crate) fn read_feature_source(&self, path: &Path) -> Result<String, FeatureIndexError> {
        let absolute = std::path::Path::new(self.path.as_str());
        let relative_path =
            path.strip_prefix(absolute)
                .map_err(|_| FeatureIndexError::OutsideWorkspaceRoot {
                    path: path.to_path_buf(),
                })?;
        if relative_path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        {
            return Err(FeatureIndexError::OutsideWorkspaceRoot {
                path: path.to_path_buf(),
            });
        }
        let relative_path =
            Utf8Path::from_path(relative_path).ok_or_else(|| FeatureIndexError::NonUtf8Path {
                path: path.to_path_buf(),
            })?;
        self.directory
            .read_to_string(relative_path)
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for the workspace-root file-read adapter boundary.

    use super::*;
    use std::ffi::OsString;
    #[cfg(not(windows))]
    use std::os::unix::ffi::OsStringExt;
    use std::path::PathBuf;

    fn open_workspace_with_feature(
        workspace: &Path,
        name: &str,
        content: &str,
    ) -> std::io::Result<PathBuf> {
        let path = workspace.join(name);
        std::fs::write(&path, content)?;
        Ok(path)
    }

    #[test]
    fn reads_feature_source_beneath_workspace_root() {
        let workspace = tempfile::tempdir().expect("temp dir");
        let path = open_workspace_with_feature(workspace.path(), "demo.feature", "Feature: demo\n")
            .expect("write feature file");
        let root = WorkspaceRoot::open(workspace.path()).expect("open workspace root");

        let source = root.read_feature_source(&path).expect("read source");

        assert_eq!(source, "Feature: demo\n");
    }

    #[test]
    fn rejects_path_outside_workspace_root() {
        let workspace = tempfile::tempdir().expect("temp dir");
        let outside = tempfile::tempdir().expect("outside temp dir");
        let path = open_workspace_with_feature(outside.path(), "outside.feature", "Feature: x\n")
            .expect("write feature file");
        let root = WorkspaceRoot::open(workspace.path()).expect("open workspace root");

        let error = root
            .read_feature_source(&path)
            .expect_err("reject outside path");

        assert!(matches!(
            error,
            FeatureIndexError::OutsideWorkspaceRoot { .. }
        ));
    }

    #[test]
    fn rejects_parent_traversal_outside_workspace_root() {
        let workspace = tempfile::tempdir().expect("temp dir");
        let path = workspace.path().join("../outside.feature");
        let root = WorkspaceRoot::open(workspace.path()).expect("open workspace root");

        let error = root
            .read_feature_source(&path)
            .expect_err("reject parent traversal");

        assert!(matches!(
            error,
            FeatureIndexError::OutsideWorkspaceRoot { .. }
        ));
    }

    #[cfg(not(windows))]
    #[test]
    fn rejects_non_utf8_relative_path() {
        let workspace = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(workspace.path().join("dir")).expect("create subdir");
        let bad = workspace
            .path()
            .join("dir")
            .join(OsString::from_vec(vec![0xff, 0xfe]));
        std::fs::write(&bad, b"Feature: x\n").expect("write feature file");
        let root = WorkspaceRoot::open(workspace.path()).expect("open workspace root");

        let error = root
            .read_feature_source(&bad)
            .expect_err("reject non-UTF-8 relative path");

        assert!(matches!(error, FeatureIndexError::NonUtf8Path { .. }));
    }
}
