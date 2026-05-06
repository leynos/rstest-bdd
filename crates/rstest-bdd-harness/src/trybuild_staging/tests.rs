//! Unit tests for [`super::copy_file`] and [`super::copy_dir_tree`] staging helpers.

use std::fs;
use std::path::PathBuf;

use rstest::fixture;
use rstest::rstest;
use tempfile::TempDir;

use super::{copy_dir_tree, copy_file};

struct CopyFileStaging {
    _root: TempDir,
    src: PathBuf,
    dst: PathBuf,
}

#[fixture]
fn copy_file_staging() -> CopyFileStaging {
    #[expect(
        clippy::expect_used,
        reason = "integration-style tests panic on improbable temp-dir I/O setup failures"
    )]
    {
        let root = TempDir::new().expect("tempdir");
        let src = root.path().join("source.txt");
        let dst = root.path().join("dest.txt");
        fs::create_dir_all(root.path()).expect("create root");
        fs::write(&src, b"new").expect("write src");
        fs::write(&dst, b"old").expect("write dst");
        CopyFileStaging {
            _root: root,
            src,
            dst,
        }
    }
}

#[rstest]
fn copy_file_overwrites_existing_destination(copy_file_staging: CopyFileStaging) {
    let CopyFileStaging { src, dst, .. } = copy_file_staging;
    #[expect(
        clippy::expect_used,
        reason = "integration-style tests panic on improbable temp-dir I/O setup failures"
    )]
    {
        copy_file(&src, &dst).expect("copy_file");
    }
    #[expect(
        clippy::expect_used,
        reason = "integration-style tests panic on improbable temp-dir I/O setup failures"
    )]
    {
        assert_eq!(fs::read(&dst).expect("read dst"), b"new");
    }
}

struct ReplaceDstStaging {
    _root: TempDir,
    src: PathBuf,
    dst: PathBuf,
}

#[expect(
    clippy::expect_used,
    reason = "integration-style tests panic on improbable temp-dir I/O setup failures"
)]
fn make_src_dst_scaffold() -> (TempDir, PathBuf, PathBuf) {
    let root = TempDir::new().expect("tempdir");
    let src = root.path().join("src");
    let dst = root.path().join("dst");
    (root, src, dst)
}

#[fixture]
fn replace_dir_staging() -> ReplaceDstStaging {
    #[expect(
        clippy::expect_used,
        reason = "integration-style tests panic on improbable temp-dir I/O setup failures"
    )]
    {
        let (root, src, dst) = make_src_dst_scaffold();
        fs::create_dir_all(src.join("sub")).expect("create src/sub");
        fs::write(src.join("sub").join("a.txt"), b"a").expect("write a.txt");
        // Pre-create destination with stale content.
        fs::create_dir_all(dst.join("stale")).expect("create stale");
        fs::write(dst.join("stale").join("old.txt"), b"old").expect("write old");
        ReplaceDstStaging {
            _root: root,
            src,
            dst,
        }
    }
}

#[rstest]
fn copy_dir_tree_replaces_existing_directory(replace_dir_staging: ReplaceDstStaging) {
    let ReplaceDstStaging { src, dst, .. } = replace_dir_staging;
    #[expect(
        clippy::expect_used,
        reason = "integration-style tests panic on improbable temp-dir I/O setup failures"
    )]
    {
        copy_dir_tree(&src, &dst).expect("copy_dir_tree");
    }
    assert!(dst.join("sub").join("a.txt").exists());
    // Stale directory must be gone.
    assert!(!dst.join("stale").exists());
}

#[fixture]
fn replace_file_dest_staging() -> ReplaceDstStaging {
    #[expect(
        clippy::expect_used,
        reason = "integration-style tests panic on improbable temp-dir I/O setup failures"
    )]
    {
        let (root, src, dst) = make_src_dst_scaffold();
        fs::create_dir_all(&src).expect("create src");
        fs::write(src.join("f.txt"), b"hello").expect("write f.txt");
        // Destination is a plain file, not a directory.
        fs::write(&dst, b"stale").expect("write stale dst");
        ReplaceDstStaging {
            _root: root,
            src,
            dst,
        }
    }
}

#[rstest]
fn copy_dir_tree_replaces_existing_file_destination(replace_file_dest_staging: ReplaceDstStaging) {
    let ReplaceDstStaging { src, dst, .. } = replace_file_dest_staging;
    #[expect(
        clippy::expect_used,
        reason = "integration-style tests panic on improbable temp-dir I/O setup failures"
    )]
    {
        copy_dir_tree(&src, &dst).expect("copy_dir_tree");
    }
    assert!(dst.join("f.txt").exists());
}

#[cfg(unix)]
struct SymlinkInSourceStaging {
    _root: TempDir,
    src: PathBuf,
    dst: PathBuf,
}

#[cfg(unix)]
#[fixture]
fn symlink_in_source_staging() -> SymlinkInSourceStaging {
    use std::os::unix::fs::symlink;

    #[expect(
        clippy::expect_used,
        reason = "integration-style tests panic on improbable temp-dir I/O setup failures"
    )]
    {
        let root = TempDir::new().expect("tempdir");
        let src = root.path().join("src");
        let dst = root.path().join("dst");
        let target = root.path().join("target.txt");
        fs::create_dir_all(&src).expect("create src");
        fs::write(&target, b"secret").expect("write target");
        symlink(&target, src.join("link.txt")).expect("symlink");
        SymlinkInSourceStaging {
            _root: root,
            src,
            dst,
        }
    }
}

#[rstest]
#[cfg(unix)]
fn copy_dir_tree_rejects_symlink_in_source(symlink_in_source_staging: SymlinkInSourceStaging) {
    let SymlinkInSourceStaging { src, dst, .. } = symlink_in_source_staging;
    #[expect(
        clippy::expect_used,
        reason = "integration-style tests panic on improbable temp-dir I/O setup failures"
    )]
    let err = { copy_dir_tree(&src, &dst).expect_err("failed to copy dir tree") };
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    assert!(
        err.to_string().contains("refusing to follow symlink"),
        "unexpected error message: {err}"
    );
}

#[test]
#[cfg(unix)]
fn copy_dir_tree_rejects_symlink_as_source_root() {
    use std::io;
    use std::os::unix::fs::symlink;

    #[expect(
        clippy::expect_used,
        reason = "integration-style tests panic on improbable temp-dir I/O setup failures"
    )]
    {
        let root = TempDir::new().expect("tempdir");
        let tree = root.path().join("tree");
        let src = root.path().join("src");
        let dst = root.path().join("dst");
        fs::create_dir_all(&tree).expect("tree dir");
        fs::write(tree.join("f.txt"), b"x").expect("write");
        symlink(&tree, &src).expect("symlink src");
        let err = copy_dir_tree(&src, &dst).expect_err("expected symlink source root rejection");
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        assert!(
            err.to_string().contains("refusing to follow symlink"),
            "unexpected error message: {err}"
        );
        assert!(
            !dst.exists(),
            "destination should not be created when source root is a symlink"
        );
    }
}
