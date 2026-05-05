//! Property tests for symlink rejection in [`super::copy_dir_tree`].

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
