//! Unit tests for [`super::copy_file`] and [`super::copy_dir_tree`] staging helpers.

use std::fs;
use std::path::{Path, PathBuf};

use rstest::fixture;
use rstest::rstest;
use tempfile::TempDir;

use super::{copy_dir_tree, copy_file};

#[expect(
    clippy::expect_used,
    reason = "integration-style tests panic on improbable temp-dir I/O setup failures"
)]
fn copy_dir_tree_ok(src: &Path, dst: &Path) {
    copy_dir_tree(src, dst).expect("copy_dir_tree");
}

#[expect(
    clippy::expect_used,
    reason = "integration-style tests panic on improbable temp-dir I/O setup failures"
)]
fn copy_file_ok(src: &Path, dst: &Path) {
    copy_file(src, dst).expect("copy_file");
}

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
    copy_file_ok(&src, &dst);
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
    copy_dir_tree_ok(&src, &dst);
    assert!(dst.join("sub").join("a.txt").exists());
    // Stale directory must be gone.
    assert!(!dst.join("stale").exists());
}

#[rstest]
fn copy_dir_tree_creates_missing_destination_parents(replace_dir_staging: ReplaceDstStaging) {
    let ReplaceDstStaging { src, dst, .. } = replace_dir_staging;
    let nested_dst = dst.join("nested").join("tree");
    copy_dir_tree_ok(&src, &nested_dst);
    assert!(nested_dst.join("sub").join("a.txt").exists());
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
    copy_dir_tree_ok(&src, &dst);
    assert!(dst.join("f.txt").exists());
}

#[test]
fn copy_dir_tree_creates_missing_destination_parent_chain() {
    #[expect(
        clippy::expect_used,
        reason = "integration-style tests panic on improbable temp-dir I/O setup failures"
    )]
    {
        let (root, src, dst) = make_src_dst_scaffold();
        let dst = dst.join("missing").join("parents");
        fs::create_dir_all(&src).expect("create src");
        fs::write(src.join("f.txt"), b"hello").expect("write f.txt");

        copy_dir_tree(&src, &dst).expect("copy_dir_tree");

        assert_eq!(
            fs::read(dst.join("f.txt")).expect("read dst file"),
            b"hello"
        );
        drop(root);
    }
}

struct MissingTailOverlapStaging {
    root: TempDir,
    src: PathBuf,
}

#[fixture]
fn missing_tail_overlap_staging() -> MissingTailOverlapStaging {
    #[expect(
        clippy::expect_used,
        reason = "integration-style tests panic on improbable temp-dir I/O setup failures"
    )]
    {
        let (root, src, _) = make_src_dst_scaffold();
        fs::create_dir_all(&src).expect("create src");
        fs::write(src.join("f.txt"), b"x").expect("write f.txt");
        MissingTailOverlapStaging { root, src }
    }
}

enum MissingTailOverlapDestination {
    InsideMissingParentChain,
    ResolvedThroughMissingParentDir,
}

impl MissingTailOverlapDestination {
    fn path(&self, root: &std::path::Path, src: &std::path::Path) -> PathBuf {
        match self {
            Self::InsideMissingParentChain => src.join("missing").join("child"),
            Self::ResolvedThroughMissingParentDir => root.join("missing").join("..").join("src"),
        }
    }
}

#[rstest]
#[case::inside_missing_parent_chain(MissingTailOverlapDestination::InsideMissingParentChain)]
#[case::resolved_through_missing_parent_dir(
    MissingTailOverlapDestination::ResolvedThroughMissingParentDir
)]
fn copy_dir_tree_rejects_missing_tail_overlap_destinations(
    missing_tail_overlap_staging: MissingTailOverlapStaging,
    #[case] destination: MissingTailOverlapDestination,
) {
    #[expect(
        clippy::expect_used,
        reason = "integration-style tests panic on improbable temp-dir I/O setup failures"
    )]
    {
        let MissingTailOverlapStaging { root, src } = missing_tail_overlap_staging;
        let dst = destination.path(root.path(), &src);
        let err = copy_dir_tree(&src, &dst).expect_err("missing-tail destination overlaps source");
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
        assert!(
            err.to_string().contains("refusing overlapping"),
            "unexpected error message: {err}"
        );
    }
}

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

#[test]
#[cfg(unix)]
fn copy_dir_tree_symlink_source_does_not_remove_destination() {
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
        fs::write(tree.join("in-tree.txt"), b"inside-tree").expect("write tree file");
        fs::create_dir_all(&dst).expect("dst dir");
        fs::write(dst.join("marker.txt"), b"untouched").expect("write dst marker");
        symlink(&tree, &src).expect("symlink src to tree");
        let err = copy_dir_tree(&src, &dst).expect_err("symlink source must be rejected");
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        assert!(
            err.to_string().contains("refusing to follow symlink"),
            "unexpected error message: {err}"
        );
        assert!(dst.is_dir(), "destination directory must still exist");
        assert_eq!(
            fs::read_to_string(dst.join("marker.txt")).expect("read marker"),
            "untouched",
            "destination contents must be unchanged (remove_destination must not run)"
        );
    }
}

#[test]
fn copy_dir_tree_rejects_identical_source_and_destination() {
    #[expect(
        clippy::expect_used,
        reason = "integration-style tests panic on improbable temp-dir I/O setup failures"
    )]
    {
        let root = TempDir::new().expect("tempdir");
        let dir = root.path().join("tree");
        fs::create_dir_all(&dir).expect("mkdir");
        let err = copy_dir_tree(&dir, &dir).expect_err("identical source and destination");
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
        assert!(
            err.to_string().contains("refusing overlapping"),
            "unexpected error message: {err}"
        );
    }
}

#[test]
fn copy_dir_tree_rejects_destination_inside_source() {
    #[expect(
        clippy::expect_used,
        reason = "integration-style tests panic on improbable temp-dir I/O setup failures"
    )]
    {
        let root = TempDir::new().expect("tempdir");
        let src = root.path().join("src");
        fs::create_dir_all(&src).expect("src");
        fs::write(src.join("f.txt"), b"x").expect("write");
        let dst = src.join("nested_dst");
        let err = copy_dir_tree(&src, &dst).expect_err("destination inside source");
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
        assert!(
            err.to_string().contains("refusing overlapping"),
            "unexpected error message: {err}"
        );
    }
}

#[test]
fn copy_dir_tree_rejects_source_inside_destination() {
    #[expect(
        clippy::expect_used,
        reason = "integration-style tests panic on improbable temp-dir I/O setup failures"
    )]
    {
        let root = TempDir::new().expect("tempdir");
        let dst = root.path().join("dst");
        fs::create_dir_all(&dst).expect("dst");
        let src = dst.join("inner_src");
        fs::create_dir_all(&src).expect("src");
        fs::write(src.join("g.txt"), b"y").expect("write");
        let err = copy_dir_tree(&src, &dst).expect_err("source inside destination");
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
        assert!(
            err.to_string().contains("refusing overlapping"),
            "unexpected error message: {err}"
        );
    }
}
