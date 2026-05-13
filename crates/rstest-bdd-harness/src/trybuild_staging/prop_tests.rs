//! Property tests for [`super::copy_dir_tree`] staging invariants.
//!
//! These cover symlink rejection as well as verification that missing
//! destination paths are canonicalized and destination overlaps are detected
//! before a copy can create or remove the wrong tree.

use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use proptest::prelude::*;

use super::{canonical_missing_destination, copy_dir_tree, paths_overlap};

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

fn path_with_missing_parent_dirs(
    root: &Path,
    existing_depth: usize,
    missing_depth: usize,
) -> PathBuf {
    let mut path = root.to_path_buf();
    for index in 0..existing_depth {
        path = path.join(format!("existing_{index}"));
    }
    for index in 0..missing_depth {
        path = path.join(format!("missing_{index}"));
    }
    path.join("dst")
}

fn path_resolving_back_to_source(root: &Path, source_name: &str, missing_depth: usize) -> PathBuf {
    let mut path = root.to_path_buf();
    for index in 0..missing_depth {
        path = path.join(format!("missing_{index}"));
    }
    for _ in 0..missing_depth {
        path = path.join("..");
    }
    path.join(source_name)
}

proptest! {
    /// Missing destination parent chains canonicalize to the same path as the
    /// nearest existing ancestor plus the generated missing tail.
    #[test]
    fn missing_destination_parent_chains_resolve_from_existing_ancestor(
        existing_depth in 0usize..5,
        missing_depth in 1usize..6,
    ) {
        let root = unique_root("missing_parent_chain");
        let _ = fs::remove_dir_all(&root);
        let destination = path_with_missing_parent_dirs(&root, existing_depth, missing_depth);
        let mut existing = root.clone();
        for index in 0..existing_depth {
            existing = existing.join(format!("existing_{index}"));
        }
        #[expect(
            clippy::expect_used,
            reason = "property-test temp-dir setup and canonicalization after explicit setup"
        )]
        {
            fs::create_dir_all(&existing).expect("create existing ancestor");
            let mut expected = fs::canonicalize(&existing).expect("canonicalize ancestor");
            for index in 0..missing_depth {
                expected = expected.join(format!("missing_{index}"));
            }
            expected = expected.join("dst");
            let actual = canonical_missing_destination(&destination);
            let _ = fs::remove_dir_all(&root);
            prop_assert_eq!(actual.expect("canonical missing destination"), expected);
        }
    }

    /// A destination with arbitrary missing components followed by matching
    /// parent-directory components still resolves back to the source tree.
    #[test]
    fn missing_tail_parent_dirs_resolve_to_overlapping_source(
        missing_depth in 1usize..6,
    ) {
        let root = unique_root("missing_tail_parent_dirs");
        let _ = fs::remove_dir_all(&root);
        let src = root.join("src");
        let destination = path_resolving_back_to_source(&root, "src", missing_depth);
        #[expect(
            clippy::expect_used,
            reason = "property-test temp-dir setup and canonicalization after explicit setup"
        )]
        {
            fs::create_dir_all(&src).expect("create src");
            let canonical_src = fs::canonicalize(&src).expect("canonicalize src");
            let canonical_dst = canonical_missing_destination(&destination)
                .expect("canonical missing destination");
            prop_assert!(paths_overlap(&canonical_src, &canonical_dst));
            prop_assert!(!root.join("missing_0").exists());
        }
        let _ = fs::remove_dir_all(&root);
    }

    /// `copy_dir_tree` rejects overlap destinations that only become apparent
    /// after resolving arbitrary missing components and `..` components.
    #[test]
    fn copy_dir_tree_rejects_generated_missing_tail_overlaps(
        missing_depth in 1usize..6,
    ) {
        let root = unique_root("copy_dir_missing_tail_overlap");
        let _ = fs::remove_dir_all(&root);
        let src = root.join("src");
        let dst = path_resolving_back_to_source(&root, "src", missing_depth);
        #[expect(
            clippy::expect_used,
            reason = "property-test temp-dir setup and err-kind extraction after explicit guards"
        )]
        {
            fs::create_dir_all(&src).expect("create src");
            fs::write(src.join("f.txt"), b"x").expect("write f.txt");
            let result = copy_dir_tree(&src, &dst);
            prop_assert!(!root.join("missing_0").exists());
            prop_assert_eq!(
                result
                    .expect_err("expected overlap rejection")
                    .kind(),
                std::io::ErrorKind::InvalidInput,
            );
        }
        let _ = fs::remove_dir_all(&root);
    }

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
        prop_assert_eq!(
            {
                #[expect(
                    clippy::expect_used,
                    reason = "property-test temp-dir setup and err-kind extraction after explicit guards"
                )]
                result
                    .expect_err("expected error from copy_dir_tree property test")
                    .kind()
            },
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
        prop_assert_eq!(
            {
                #[expect(
                    clippy::expect_used,
                    reason = "property-test temp-dir setup and err-kind extraction after explicit guards"
                )]
                result
                    .expect_err("expected error from copy_dir_tree property test")
                    .kind()
            },
            std::io::ErrorKind::InvalidInput,
        );
    }
}
