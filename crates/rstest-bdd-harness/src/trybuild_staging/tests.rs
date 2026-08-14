//! Unit tests for [`super::copy_file`] and [`super::copy_dir_tree`] staging helpers.

use std::fs;
use std::io;
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
fn copy_file_staging() -> io::Result<CopyFileStaging> {
    let root = TempDir::new()?;
    let src = root.path().join("source.txt");
    let dst = root.path().join("dest.txt");
    fs::write(&src, b"new")?;
    fs::write(&dst, b"old")?;
    Ok(CopyFileStaging {
        _root: root,
        src,
        dst,
    })
}

#[rstest]
fn copy_file_overwrites_existing_destination(copy_file_staging: io::Result<CopyFileStaging>) {
    let staging = copy_file_staging.expect("test setup should succeed");
    let CopyFileStaging { src, dst, .. } = &staging;
    copy_file(src, dst).expect("test setup should succeed");
    assert_eq!(fs::read(dst).expect("test setup should succeed"), b"new");
}

struct ReplaceDstStaging {
    _root: TempDir,
    src: PathBuf,
    dst: PathBuf,
}

fn make_src_dst_scaffold() -> io::Result<(TempDir, PathBuf, PathBuf)> {
    let root = TempDir::new()?;
    let src = root.path().join("src");
    let dst = root.path().join("dst");
    Ok((root, src, dst))
}

struct OverlapCheckStaging {
    root: TempDir,
    src: PathBuf,
}

#[fixture]
fn overlap_check_staging() -> io::Result<OverlapCheckStaging> {
    let (root, src, _dst) = make_src_dst_scaffold()?;
    fs::create_dir_all(&src)?;
    fs::write(src.join("f.txt"), b"x")?;
    Ok(OverlapCheckStaging { root, src })
}

#[fixture]
fn replace_dir_staging() -> io::Result<ReplaceDstStaging> {
    let (root, src, dst) = make_src_dst_scaffold()?;
    fs::create_dir_all(src.join("sub"))?;
    fs::write(src.join("sub").join("a.txt"), b"a")?;
    // Pre-create destination with stale content.
    fs::create_dir_all(dst.join("stale"))?;
    fs::write(dst.join("stale").join("old.txt"), b"old")?;
    Ok(ReplaceDstStaging {
        _root: root,
        src,
        dst,
    })
}

#[rstest]
fn copy_dir_tree_replaces_existing_directory(replace_dir_staging: io::Result<ReplaceDstStaging>) {
    let staging = replace_dir_staging.expect("test setup should succeed");
    let ReplaceDstStaging { src, dst, .. } = &staging;
    copy_dir_tree(src, dst).expect("test setup should succeed");
    assert!(dst.join("sub").join("a.txt").exists());
    // Stale directory must be gone.
    assert!(!dst.join("stale").exists());
}

#[rstest]
fn copy_dir_tree_creates_missing_destination_parents(
    replace_dir_staging: io::Result<ReplaceDstStaging>,
) {
    let staging = replace_dir_staging.expect("test setup should succeed");
    let ReplaceDstStaging { src, dst, .. } = &staging;
    let nested_dst = dst.join("nested").join("tree");
    copy_dir_tree(src, &nested_dst).expect("test setup should succeed");
    assert!(nested_dst.join("sub").join("a.txt").exists());
}

#[fixture]
fn replace_file_dest_staging() -> io::Result<ReplaceDstStaging> {
    let (root, src, dst) = make_src_dst_scaffold()?;
    fs::create_dir_all(&src)?;
    fs::write(src.join("f.txt"), b"hello")?;
    // Destination is a plain file, not a directory.
    fs::write(&dst, b"stale")?;
    Ok(ReplaceDstStaging {
        _root: root,
        src,
        dst,
    })
}

#[rstest]
fn copy_dir_tree_replaces_existing_file_destination(
    replace_file_dest_staging: io::Result<ReplaceDstStaging>,
) {
    let staging = replace_file_dest_staging.expect("test setup should succeed");
    let ReplaceDstStaging { src, dst, .. } = &staging;
    copy_dir_tree(src, dst).expect("test setup should succeed");
    assert!(dst.join("f.txt").exists());
}

#[test]
fn copy_dir_tree_creates_missing_destination_parent_chain() {
    let (root, src, dst) = make_src_dst_scaffold().expect("test setup should succeed");
    let dst = dst.join("missing").join("parents");
    fs::create_dir_all(&src).expect("test setup should succeed");
    fs::write(src.join("f.txt"), b"hello").expect("test setup should succeed");

    copy_dir_tree(&src, &dst).expect("test setup should succeed");

    assert_eq!(
        fs::read(dst.join("f.txt")).expect("test setup should succeed"),
        b"hello"
    );
    drop(root);
}

#[derive(Clone)]
enum MissingTailDestination {
    InsideSource,
    ResolvedBackToSource,
}

#[rstest]
#[case::inside_source(MissingTailDestination::InsideSource)]
#[case::resolved_back_to_source(MissingTailDestination::ResolvedBackToSource)]
fn copy_dir_tree_rejects_missing_tail_overlap_destinations(
    overlap_check_staging: io::Result<OverlapCheckStaging>,
    #[case] variant: MissingTailDestination,
) -> io::Result<()> {
    let staging = overlap_check_staging?;
    let OverlapCheckStaging { root, src } = &staging;
    let missing = root.path().join("missing");
    let (dst, not_created) = match variant {
        MissingTailDestination::InsideSource => {
            let d = src.join("missing").join("child");
            (d.clone(), d)
        }
        MissingTailDestination::ResolvedBackToSource => (missing.join("..").join("src"), missing),
    };
    let err = match copy_dir_tree(src, &dst) {
        Ok(()) => panic!("expected overlap rejection"),
        Err(err) => err,
    };
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    assert!(!not_created.exists(), "no new path should be created");
    assert!(
        err.to_string().contains("refusing overlapping"),
        "unexpected error message: {err}"
    );
    Ok(())
}

#[cfg(unix)]
struct SymlinkInSourceStaging {
    _root: TempDir,
    src: PathBuf,
    dst: PathBuf,
}

#[cfg(unix)]
#[fixture]
fn symlink_in_source_staging() -> io::Result<SymlinkInSourceStaging> {
    use std::os::unix::fs::symlink;

    let root = TempDir::new()?;
    let src = root.path().join("src");
    let dst = root.path().join("dst");
    let target = root.path().join("target.txt");
    fs::create_dir_all(&src)?;
    fs::write(&target, b"secret")?;
    symlink(&target, src.join("link.txt"))?;
    Ok(SymlinkInSourceStaging {
        _root: root,
        src,
        dst,
    })
}

#[rstest]
#[cfg(unix)]
fn copy_dir_tree_rejects_symlink_in_source(
    symlink_in_source_staging: io::Result<SymlinkInSourceStaging>,
) {
    let staging = symlink_in_source_staging.expect("test setup should succeed");
    let SymlinkInSourceStaging { src, dst, .. } = &staging;
    let err = { copy_dir_tree(src, dst).expect_err("failed to copy dir tree") };
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    assert!(
        err.to_string().contains("refusing to follow symlink"),
        "unexpected error message: {err}"
    );
}

#[test]
#[cfg(unix)]
fn copy_dir_tree_rejects_symlink_as_source_root() {
    use std::os::unix::fs::symlink;

    let root = TempDir::new().expect("test setup should succeed");
    let tree = root.path().join("tree");
    let src = root.path().join("src");
    let dst = root.path().join("dst");
    fs::create_dir_all(&tree).expect("test setup should succeed");
    fs::write(tree.join("f.txt"), b"x").expect("test setup should succeed");
    symlink(&tree, &src).expect("test setup should succeed");
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

#[test]
#[cfg(unix)]
fn copy_dir_tree_symlink_source_does_not_remove_destination() {
    use std::os::unix::fs::symlink;

    let root = TempDir::new().expect("test setup should succeed");
    let tree = root.path().join("tree");
    let src = root.path().join("src");
    let dst = root.path().join("dst");
    fs::create_dir_all(&tree).expect("test setup should succeed");
    fs::write(tree.join("in-tree.txt"), b"inside-tree").expect("test setup should succeed");
    fs::create_dir_all(&dst).expect("test setup should succeed");
    fs::write(dst.join("marker.txt"), b"untouched").expect("test setup should succeed");
    symlink(&tree, &src).expect("test setup should succeed");
    let err = copy_dir_tree(&src, &dst).expect_err("symlink source must be rejected");
    assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    assert!(
        err.to_string().contains("refusing to follow symlink"),
        "unexpected error message: {err}"
    );
    assert!(dst.is_dir(), "destination directory must still exist");
    assert_eq!(
        fs::read_to_string(dst.join("marker.txt")).expect("test setup should succeed"),
        "untouched",
        "destination contents must be unchanged (remove_destination must not run)"
    );
}

#[test]
fn copy_dir_tree_rejects_identical_source_and_destination() {
    let root = TempDir::new().expect("test setup should succeed");
    let dir = root.path().join("tree");
    fs::create_dir_all(&dir).expect("test setup should succeed");
    let err = copy_dir_tree(&dir, &dir).expect_err("identical source and destination");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    assert!(
        err.to_string().contains("refusing overlapping"),
        "unexpected error message: {err}"
    );
}

#[test]
fn copy_dir_tree_rejects_destination_inside_source() {
    let root = TempDir::new().expect("test setup should succeed");
    let src = root.path().join("src");
    fs::create_dir_all(&src).expect("test setup should succeed");
    fs::write(src.join("f.txt"), b"x").expect("test setup should succeed");
    let dst = src.join("nested_dst");
    let err = copy_dir_tree(&src, &dst).expect_err("destination inside source");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    assert!(
        err.to_string().contains("refusing overlapping"),
        "unexpected error message: {err}"
    );
}

#[test]
fn copy_dir_tree_rejects_source_inside_destination() {
    let root = TempDir::new().expect("test setup should succeed");
    let dst = root.path().join("dst");
    fs::create_dir_all(&dst).expect("test setup should succeed");
    let src = dst.join("inner_src");
    fs::create_dir_all(&src).expect("test setup should succeed");
    fs::write(src.join("g.txt"), b"y").expect("test setup should succeed");
    let err = copy_dir_tree(&src, &dst).expect_err("source inside destination");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    assert!(
        err.to_string().contains("refusing overlapping"),
        "unexpected error message: {err}"
    );
}
