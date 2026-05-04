//! Filesystem helpers for harness crates that run `trybuild` against fixtures.
//!
//! These utilities exist so Tokio and GPUI harness integration tests can stage
//! support files into Cargo's trybuild scratch directory with identical
//! behaviour. They are exposed as a hidden module for those tests only and are
//! not part of the supported public API.

use std::fs;
use std::io;
use std::path::Path;

/// Copies a single file, creating parent directories as needed.
pub fn copy_file(source: &Path, destination: &Path) -> io::Result<()> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(source, destination).map(|_| ())
}

/// Recursively copies a directory tree, replacing `destination` if it exists.
pub fn copy_dir_tree(source: &Path, destination: &Path) -> io::Result<()> {
    if destination.exists() {
        fs::remove_dir_all(destination)?;
    }
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir_tree(&source_path, &destination_path)?;
        } else {
            copy_file(&source_path, &destination_path)?;
        }
    }
    Ok(())
}
