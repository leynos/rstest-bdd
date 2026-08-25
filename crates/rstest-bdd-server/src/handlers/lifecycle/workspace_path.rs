//! Workspace-path selection for LSP initialization requests.

use std::path::PathBuf;

use lsp_types::{Url, WorkspaceFolder};

/// Extract a workspace path from the first workspace folder.
///
/// If that folder is not a file URI, or no folders are provided, the root URI
/// is used instead.
pub(super) fn extract_workspace_path(
    workspace_folders: &[WorkspaceFolder],
    root_uri: Option<&Url>,
) -> Option<PathBuf> {
    workspace_folders
        .first()
        .and_then(|folder| url_to_path(&folder.uri))
        .or_else(|| root_uri.and_then(url_to_path))
}

/// Convert a URL to a file system path.
///
/// Only handles `file://` URLs; returns `None` for other schemes.
pub(super) fn url_to_path(url: &Url) -> Option<PathBuf> { url.to_file_path().ok() }
