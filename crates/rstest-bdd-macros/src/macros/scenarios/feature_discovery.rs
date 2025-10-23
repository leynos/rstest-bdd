//! Discovers `.feature` files for the `scenarios!` macro without following
//! symlink targets. Delegates path canonicalisation to keep diagnostics
//! stable even when fixtures reside outside the workspace tree.

use std::path::{Path, PathBuf};

use walkdir::{DirEntry, WalkDir};

use super::path_resolution::canonicalize_path;

fn is_feature_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("feature"))
}

/// Process a regular file entry, returning it if it's a feature file.
fn process_regular_file(path: PathBuf) -> Option<std::io::Result<PathBuf>> {
    if is_feature_file(&path) {
        Some(Ok(path))
    } else {
        None
    }
}

/// Process a special entry (e.g., symlink) by canonicalising and checking if
/// it's a feature file.
fn process_special_entry(original_path: PathBuf) -> Option<std::io::Result<PathBuf>> {
    match canonicalize_path(&original_path) {
        Ok(real_path) if real_path.is_file() && is_feature_file(&real_path) => {
            Some(Ok(original_path))
        }
        Ok(_) => None,
        Err(err) => Some(Err(err)),
    }
}

fn process_dir_entry(entry: DirEntry) -> Option<std::io::Result<PathBuf>> {
    let file_type = entry.file_type();
    if file_type.is_dir() {
        return None;
    }

    let path = entry.into_path();

    if file_type.is_file() {
        return process_regular_file(path);
    }

    process_special_entry(path)
}

fn convert_walkdir_error(err: walkdir::Error) -> Option<std::io::Error> {
    if err.loop_ancestor().is_some() {
        return None;
    }

    let err_str = err.to_string();
    Some(
        err.into_io_error()
            .unwrap_or_else(|| std::io::Error::other(err_str)),
    )
}

pub(super) fn collect_feature_files(base: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    for next in WalkDir::new(base).follow_links(false) {
        match next {
            Ok(entry) => {
                if let Some(result) = process_dir_entry(entry) {
                    files.push(result?);
                }
            }
            Err(err) => {
                if let Some(err) = convert_walkdir_error(err) {
                    return Err(err);
                }
            }
        }
    }

    files.sort();
    Ok(files)
}
