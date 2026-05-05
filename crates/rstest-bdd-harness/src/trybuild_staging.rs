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
///
/// ```
/// use std::fs;
/// use std::time::{SystemTime, UNIX_EPOCH};
///
/// use rstest_bdd_harness::trybuild_staging::copy_file;
///
/// let root = std::env::temp_dir().join(format!(
///     "{}_{}_{}",
///     concat!("rstest_bdd_trybuild_staging_copy_file_doc_", env!("CARGO_PKG_NAME")),
///     std::process::id(),
///     SystemTime::now()
///         .duration_since(UNIX_EPOCH)
///         .expect("system clock before UNIX epoch")
///         .as_nanos(),
/// ));
/// let _ = fs::remove_dir_all(&root);
/// fs::create_dir_all(&root).expect("failed to create dir");
/// let source = root.join("source.txt");
/// let destination = root.join("nested").join("dest.txt");
/// fs::write(&source, b"needle").expect("failed to write source");
/// copy_file(&source, &destination).expect("failed to copy file");
/// assert!(destination.exists());
/// assert_eq!(
///     fs::read(&destination).expect("failed to read destination"),
///     b"needle"
/// );
/// let _ = fs::remove_dir_all(&root);
/// ```
pub fn copy_file(source: &Path, destination: &Path) -> io::Result<()> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(source, destination).map(|_| ())
}

/// Recursively copies a directory tree, replacing `destination` if it exists.
///
/// Symlinks under `source` are rejected so a malicious or accidental link cannot
/// escape the tree or create copy loops.
///
/// ```
/// use std::fs;
/// use std::time::{SystemTime, UNIX_EPOCH};
///
/// use rstest_bdd_harness::trybuild_staging::copy_dir_tree;
///
/// let root = std::env::temp_dir().join(format!(
///     "{}_{}_{}",
///     concat!("rstest_bdd_trybuild_staging_copy_dir_doc_", env!("CARGO_PKG_NAME")),
///     std::process::id(),
///     SystemTime::now()
///         .duration_since(UNIX_EPOCH)
///         .expect("system clock before UNIX epoch")
///         .as_nanos(),
/// ));
/// let _ = fs::remove_dir_all(&root);
/// let src = root.join("src");
/// fs::create_dir_all(src.join("inner")).expect("failed to create dir");
/// fs::write(src.join("inner").join("note.txt"), b"tree").expect("failed to write source");
/// let dst = root.join("dst");
/// copy_dir_tree(&src, &dst).expect("failed to copy dir tree");
/// let out = dst.join("inner").join("note.txt");
/// assert!(out.exists());
/// assert_eq!(
///     fs::read(&out).expect("failed to read destination"),
///     b"tree"
/// );
/// let _ = fs::remove_dir_all(&root);
/// ```
pub fn copy_dir_tree(source: &Path, destination: &Path) -> io::Result<()> {
    match fs::symlink_metadata(destination) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
        Ok(metadata) => {
            let file_type = metadata.file_type();
            if file_type.is_symlink() {
                fs::remove_file(destination)?;
            } else if file_type.is_dir() {
                fs::remove_dir_all(destination)?;
            } else {
                fs::remove_file(destination)?;
            }
        }
    }
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "refusing to follow symlink while staging trybuild fixtures: {}",
                    entry.path().display()
                ),
            ));
        }
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_tree(&source_path, &destination_path)?;
        } else {
            copy_file(&source_path, &destination_path)?;
        }
    }
    Ok(())
}
