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

fn remove_destination(destination: &Path) -> io::Result<()> {
    match fs::symlink_metadata(destination) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
        Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(destination),
        Ok(_) => fs::remove_file(destination),
    }
}

fn copy_entry(entry: &fs::DirEntry, destination: &Path) -> io::Result<()> {
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
/// let err = copy_dir_tree(&src, &dst).unwrap_err();
/// assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
/// let _ = fs::remove_dir_all(&root);
/// # }
/// ```
pub fn copy_dir_tree(source: &Path, destination: &Path) -> io::Result<()> {
    remove_destination(destination)?;
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        copy_entry(&entry?, destination)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{copy_dir_tree, copy_file};

    #[expect(
        clippy::expect_used,
        reason = "integration-style tests panic on improbable temp-dir I/O setup failures"
    )]
    fn unique_root(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "rstest_bdd_trybuild_staging_{}_{}_{}_{}",
            label,
            env!("CARGO_PKG_NAME"),
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock before epoch")
                .as_nanos(),
        ))
    }

    #[test]
    fn copy_file_overwrites_existing_destination() {
        let root = unique_root("overwrite");
        let _ = fs::remove_dir_all(&root);
        let src = root.join("source.txt");
        let dst = root.join("dest.txt");
        #[expect(
            clippy::expect_used,
            reason = "integration-style tests panic on improbable temp-dir I/O setup failures"
        )]
        {
            fs::create_dir_all(&root).expect("create root");
            fs::write(&src, b"new").expect("write src");
            fs::write(&dst, b"old").expect("write dst");
            copy_file(&src, &dst).expect("copy_file");
        }
        #[expect(
            clippy::expect_used,
            reason = "integration-style tests panic on improbable temp-dir I/O setup failures"
        )]
        {
            assert_eq!(fs::read(&dst).expect("read dst"), b"new");
        }
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn copy_dir_tree_replaces_existing_directory() {
        let root = unique_root("replace_dir");
        let _ = fs::remove_dir_all(&root);
        let src = root.join("src");
        let dst = root.join("dst");
        #[expect(
            clippy::expect_used,
            reason = "integration-style tests panic on improbable temp-dir I/O setup failures"
        )]
        {
            fs::create_dir_all(src.join("sub")).expect("create src/sub");
            fs::write(src.join("sub").join("a.txt"), b"a").expect("write a.txt");
            // Pre-create destination with stale content.
            fs::create_dir_all(dst.join("stale")).expect("create stale");
            fs::write(dst.join("stale").join("old.txt"), b"old").expect("write old");
            copy_dir_tree(&src, &dst).expect("copy_dir_tree");
        }
        assert!(dst.join("sub").join("a.txt").exists());
        // Stale directory must be gone.
        assert!(!dst.join("stale").exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn copy_dir_tree_replaces_existing_file_destination() {
        let root = unique_root("replace_file");
        let _ = fs::remove_dir_all(&root);
        let src = root.join("src");
        let dst = root.join("dst");
        #[expect(
            clippy::expect_used,
            reason = "integration-style tests panic on improbable temp-dir I/O setup failures"
        )]
        {
            fs::create_dir_all(&src).expect("create src");
            fs::write(src.join("f.txt"), b"hello").expect("write f.txt");
            // Destination is a plain file, not a directory.
            fs::create_dir_all(&root).expect("ensure root exists");
            fs::write(&dst, b"stale").expect("write stale dst");
            copy_dir_tree(&src, &dst).expect("copy_dir_tree");
        }
        assert!(dst.join("f.txt").exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    #[cfg(unix)]
    fn copy_dir_tree_rejects_symlink_in_source() {
        use std::os::unix::fs::symlink;
        let root = unique_root("symlink_reject");
        let _ = fs::remove_dir_all(&root);
        let src = root.join("src");
        let dst = root.join("dst");
        let target = root.join("target.txt");
        #[expect(
            clippy::expect_used,
            reason = "integration-style tests panic on improbable temp-dir I/O setup failures"
        )]
        {
            fs::create_dir_all(&src).expect("create src");
            fs::write(&target, b"secret").expect("write target");
            symlink(&target, src.join("link.txt")).expect("symlink");
        }
        #[expect(
            clippy::unwrap_used,
            reason = "integration-style tests panic on improbable temp-dir I/O setup failures"
        )]
        let err = { copy_dir_tree(&src, &dst).unwrap_err() };
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
        assert!(
            err.to_string().contains("refusing to follow symlink"),
            "unexpected error message: {err}"
        );
        let _ = fs::remove_dir_all(&root);
    }
}

#[cfg(all(test, unix))]
mod prop_tests {
    use std::fs;
    use std::os::unix::fs::symlink;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use proptest::prelude::*;

    use super::copy_dir_tree;

    #[expect(
        clippy::expect_used,
        reason = "property-test temp-dir setup and err-kind extraction after explicit guards"
    )]
    fn unique_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "rstest_bdd_prop_{}_{}_{}_{}",
            label,
            env!("CARGO_PKG_NAME"),
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock before epoch")
                .as_nanos(),
        ))
    }

    /// Creates the shared directory scaffold for symlink-rejection property tests.
    ///
    /// Returns `(src, dst, target)` where `src/` exists on disk and `target.txt`
    /// has been written; `dst` is the intended copy destination (not yet created).
    #[expect(
        clippy::expect_used,
        reason = "property-test temp-dir setup and err-kind extraction after explicit guards"
    )]
    fn setup_source_scaffold(root: &Path) -> (PathBuf, PathBuf, PathBuf) {
        let src = root.join("src");
        let dst = root.join("dst");
        let target = root.join("target.txt");
        fs::create_dir_all(&src).expect("create src");
        fs::write(&target, b"secret").expect("write target");
        (src, dst, target)
    }

    /// Creates `src/` containing plain files for each name in `file_names` and
    /// a symlink named `symlink_name` pointing at a file outside the tree.
    #[expect(
        clippy::expect_used,
        reason = "property-test temp-dir setup and err-kind extraction after explicit guards"
    )]
    fn build_flat_source_with_symlink(
        root: &Path,
        file_names: &[String],
        symlink_name: &str,
    ) -> (PathBuf, PathBuf) {
        let (src, dst, target) = setup_source_scaffold(root);
        for name in file_names {
            fs::write(src.join(name), b"data").expect("write file");
        }
        symlink(&target, src.join(symlink_name)).expect("create symlink");
        (src, dst)
    }

    /// Creates `src/` with a chain of subdirectories `depth` levels deep,
    /// placing the symlink at the innermost level.
    #[expect(
        clippy::expect_used,
        reason = "property-test temp-dir setup and err-kind extraction after explicit guards"
    )]
    fn build_nested_source_with_symlink(
        root: &Path,
        depth: usize,
        symlink_name: &str,
    ) -> (PathBuf, PathBuf) {
        let (src, dst, target) = setup_source_scaffold(root);
        let mut sub = src.clone();
        for i in 0..depth {
            sub = sub.join(format!("level_{i}"));
        }
        fs::create_dir_all(&sub).expect("create nested dir");
        symlink(&target, sub.join(symlink_name)).expect("create symlink");
        (src, dst)
    }

    proptest! {
        /// A symlink at the top level of the source tree is always rejected,
        /// regardless of how many regular files accompany it.
        #[test]
        fn symlink_at_top_level_is_always_rejected(
            file_names in prop::collection::vec(
                "[a-z][a-z0-9]{0,7}\\.txt",
                0..8usize,
            ),
            symlink_name in "[a-z][a-z0-9]{0,7}\\.lnk",
        ) {
            let root = unique_root("top_level");
            let _ = fs::remove_dir_all(&root);
            let (src, dst) = build_flat_source_with_symlink(
                &root,
                &file_names,
                &symlink_name,
            );
            let result = copy_dir_tree(&src, &dst);
            let _ = fs::remove_dir_all(&root);
            prop_assert!(result.is_err());
            prop_assert_eq!(
                {
                    #[expect(
                        clippy::unwrap_used,
                        reason = "property-test temp-dir setup and err-kind extraction after explicit guards"
                    )]
                    result.unwrap_err()
                }
                .kind(),
                std::io::ErrorKind::InvalidInput,
            );
        }

        /// A symlink nested at an arbitrary depth within the source tree is
        /// always rejected, verifying the invariant holds through recursion.
        #[test]
        fn symlink_at_arbitrary_depth_is_always_rejected(
            depth in 1usize..6,
            symlink_name in "[a-z][a-z0-9]{0,7}\\.lnk",
        ) {
            let root = unique_root("nested");
            let _ = fs::remove_dir_all(&root);
            let (src, dst) = build_nested_source_with_symlink(
                &root,
                depth,
                &symlink_name,
            );
            let result = copy_dir_tree(&src, &dst);
            let _ = fs::remove_dir_all(&root);
            prop_assert!(result.is_err());
            prop_assert_eq!(
                {
                    #[expect(
                        clippy::unwrap_used,
                        reason = "property-test temp-dir setup and err-kind extraction after explicit guards"
                    )]
                    result.unwrap_err()
                }
                .kind(),
                std::io::ErrorKind::InvalidInput,
            );
        }
    }
}
