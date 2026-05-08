//! Filesystem helpers for harness crates that run `trybuild` against fixtures.
//!
//! These utilities exist so Tokio and GPUI harness integration tests can stage
//! support files into Cargo's trybuild scratch directory with identical
//! behaviour. They are exposed as a hidden module for those tests only and are
//! not part of the supported public API.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[cfg(test)]
mod tests;

#[cfg(all(test, unix))]
mod prop_tests;

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

pub(super) fn remove_destination(destination: &Path) -> io::Result<()> {
    match fs::symlink_metadata(destination) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
        Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(destination),
        Ok(_) => fs::remove_file(destination),
    }
}

pub(super) fn copy_entry(entry: &fs::DirEntry, destination: &Path) -> io::Result<()> {
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
    let destination_path = destination.join(entry.file_name());
    if file_type.is_dir() {
        copy_dir_tree(&entry.path(), &destination_path)
    } else {
        copy_file(&entry.path(), &destination_path)
    }
}

fn canonical_destination_for_overlap(destination: &Path) -> io::Result<PathBuf> {
    match fs::canonicalize(destination) {
        Ok(path) => Ok(path),
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            canonical_missing_destination(destination)
        }
        Err(err) => Err(err),
    }
}

fn append_missing_components(mut base: PathBuf, missing: &[std::ffi::OsString]) -> PathBuf {
    for component in missing.iter().rev() {
        base.push(component);
    }
    base
}

fn canonical_missing_destination(destination: &Path) -> io::Result<PathBuf> {
    let mut missing_components: Vec<std::ffi::OsString> = Vec::new();

    for ancestor in destination.ancestors() {
        if ancestor.as_os_str().is_empty() {
            break;
        }
        match fs::canonicalize(ancestor) {
            Ok(base) => return Ok(append_missing_components(base, &missing_components)),
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                if let Some(name) = ancestor.file_name() {
                    missing_components.push(name.to_os_string());
                }
            }
            Err(err) => return Err(err),
        }
    }

    // No existing ancestor found; resolve against the current working directory.
    let base = fs::canonicalize(std::env::current_dir()?)?;
    Ok(append_missing_components(base, &missing_components))
}

fn paths_overlap(a: &Path, b: &Path) -> bool {
    a == b || a.starts_with(b) || b.starts_with(a)
}

/// Rejects paths where `copy_dir_tree` would call `remove_destination` on a tree
/// that still contains (or equals) the source directory.
fn reject_overlapping_copy_dir_tree_paths(source: &Path, destination: &Path) -> io::Result<()> {
    let canonical_source = fs::canonicalize(source)?;
    let canonical_destination = canonical_destination_for_overlap(destination)?;
    if paths_overlap(&canonical_source, &canonical_destination) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "refusing overlapping source {} and destination {} for copy_dir_tree: \
                 remove_destination would delete paths still required by the source tree",
                source.display(),
                destination.display(),
            ),
        ));
    }
    Ok(())
}

/// Recursively copies a directory tree, replacing `destination` if it exists.
///
/// The `source` path itself and any symlinks beneath it are rejected so a
/// malicious or accidental link cannot escape the tree or create copy loops.
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
///
/// A symlink anywhere in the source tree is rejected:
///
/// ```
/// # #[cfg(unix)]
/// # {
/// use std::fs;
/// use std::time::{SystemTime, UNIX_EPOCH};
///
/// use rstest_bdd_harness::trybuild_staging::copy_dir_tree;
///
/// let root = std::env::temp_dir().join(format!(
///     "{}_{}_{}",
///     concat!(
///         "rstest_bdd_trybuild_staging_copy_dir_symlink_doc_",
///         env!("CARGO_PKG_NAME")
///     ),
///     std::process::id(),
///     SystemTime::now()
///         .duration_since(UNIX_EPOCH)
///         .expect("system clock before UNIX epoch")
///         .as_nanos(),
/// ));
/// let _ = fs::remove_dir_all(&root);
/// let src = root.join("src");
/// let dst = root.join("dst");
/// let target = root.join("escape.txt");
/// fs::create_dir_all(&src).expect("failed to create src");
/// fs::write(&target, b"secret").expect("failed to write target");
/// std::os::unix::fs::symlink(&target, src.join("link.txt"))
///     .expect("failed to create symlink");
/// let err = copy_dir_tree(&src, &dst).expect_err("copy_dir_tree should reject symlink");
/// assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
/// let _ = fs::remove_dir_all(&root);
/// # }
/// ```
pub fn copy_dir_tree(source: &Path, destination: &Path) -> io::Result<()> {
    let source_meta = fs::symlink_metadata(source)?;
    if source_meta.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "refusing to follow symlink while staging trybuild fixtures: {}",
                source.display()
            ),
        ));
    }
    reject_overlapping_copy_dir_tree_paths(source, destination)?;
    let entries = fs::read_dir(source)?;
    remove_destination(destination)?;
    fs::create_dir_all(destination)?;
    for entry in entries {
        copy_entry(&entry?, destination)?;
    }
    Ok(())
}
