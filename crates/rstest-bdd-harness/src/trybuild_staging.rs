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
///
/// use rstest_bdd_harness::trybuild_staging::copy_file;
///
/// let root = std::env::temp_dir().join(concat!(
///     "rstest_bdd_trybuild_staging_copy_file_doc",
///     "_",
///     env!("CARGO_PKG_NAME"),
/// ));
/// let _ = fs::remove_dir_all(&root);
/// fs::create_dir_all(&root).unwrap();
/// let source = root.join("source.txt");
/// let destination = root.join("nested").join("dest.txt");
/// fs::write(&source, b"needle").unwrap();
/// copy_file(&source, &destination).unwrap();
/// assert!(destination.exists());
/// assert_eq!(fs::read(&destination).unwrap(), b"needle");
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
///
/// use rstest_bdd_harness::trybuild_staging::copy_dir_tree;
///
/// let root = std::env::temp_dir().join(concat!(
///     "rstest_bdd_trybuild_staging_copy_dir_doc",
///     "_",
///     env!("CARGO_PKG_NAME"),
/// ));
/// let _ = fs::remove_dir_all(&root);
/// let src = root.join("src");
/// fs::create_dir_all(src.join("inner")).unwrap();
/// fs::write(src.join("inner").join("note.txt"), b"tree").unwrap();
/// let dst = root.join("dst");
/// copy_dir_tree(&src, &dst).unwrap();
/// let out = dst.join("inner").join("note.txt");
/// assert!(out.exists());
/// assert_eq!(fs::read(&out).unwrap(), b"tree");
/// let _ = fs::remove_dir_all(&root);
/// ```
pub fn copy_dir_tree(source: &Path, destination: &Path) -> io::Result<()> {
    if destination.exists() {
        fs::remove_dir_all(destination)?;
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
