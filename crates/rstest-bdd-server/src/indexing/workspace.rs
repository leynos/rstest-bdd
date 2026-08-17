//! Capability-scoped workspace-root access for feature indexing.
//!
//! This module owns the validated root capability shared by `ServerState` and
//! disk-backed feature indexing. The server retains the capability, while the
//! indexer uses it only to read paths that have been validated as root-relative.

use std::path::Path;

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;

use crate::error::ServerError;

/// Validated capability for reading files beneath one workspace root.
pub(crate) struct WorkspaceRoot {
    path: Utf8PathBuf,
    directory: Dir,
}

impl WorkspaceRoot {
    pub(crate) fn open(path: &Path) -> Result<Self, ServerError> {
        let path = Utf8Path::from_path(path)
            .ok_or_else(|| ServerError::WorkspaceRootNotUtf8(path.to_path_buf()))?
            .to_path_buf();
        let directory = Dir::open_ambient_dir(&path, ambient_authority())?;

        Ok(Self { path, directory })
    }

    pub(crate) fn path(&self) -> &Utf8Path {
        &self.path
    }

    pub(crate) fn directory(&self) -> &Dir {
        &self.directory
    }
}
