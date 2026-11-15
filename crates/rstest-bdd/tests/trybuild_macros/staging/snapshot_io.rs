//! Locating and reading the trybuild snapshots that staging rewrites.
use std::io;

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::fs::Dir;

use super::TARGET_ROOT_SNAPSHOTS;

/// The workspace root, two directories above the crate root.
pub(super) fn workspace_root_of(crate_root: &Utf8Path) -> io::Result<&Utf8Path> {
    crate_root
        .parent()
        .and_then(Utf8Path::parent)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "workspace root must exist"))
}

/// Read every snapshot's pristine contents so the guard can restore them.
pub(super) fn read_snapshot_originals(crate_dir: &Dir) -> io::Result<Vec<(Utf8PathBuf, String)>> {
    TARGET_ROOT_SNAPSHOTS
        .iter()
        .map(|path| {
            let path = Utf8PathBuf::from(path);
            let original = crate_dir.read_to_string(path.as_std_path())?;
            Ok((path, original))
        })
        .collect()
}
