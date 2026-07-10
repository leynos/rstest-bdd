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
fn copy_file_overwrites_existing_destination(
    copy_file_staging: io::Result<CopyFileStaging>,
) -> io::Result<()> {
    let staging = copy_file_staging?;
    let CopyFileStaging { src, dst, .. } = &staging;
    copy_file(src, dst)?;
    assert_eq!(fs::read(dst)?, b"new");
    Ok(())
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
fn copy_dir_tree_replaces_existing_directory(
    replace_dir_staging: io::Result<ReplaceDstStaging>,
) -> io::Result<()> {
    let staging = replace_dir_staging?;
    let ReplaceDstStaging { src, dst, .. } = &staging;
    copy_dir_tree(src, dst)?;
    assert!(dst.join("sub").join("a.txt").exists());
    // Stale directory must be gone.
    assert!(!dst.join("stale").exists());
    Ok(())
}

#[rstest]
fn copy_dir_tree_creates_missing_destination_parents(
    replace_dir_staging: io::Result<ReplaceDstStaging>,
) -> io::Result<()> {
    let staging = replace_dir_staging?;
    let ReplaceDstStaging { src, dst, .. } = &staging;
    let nested_dst = dst.join("nested").join("tree");
    copy_dir_tree(src, &nested_dst)?;
    assert!(nested_dst.join("sub").join("a.txt").exists());
    Ok(())
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
) -> io::Result<()> {
    let staging = replace_file_dest_staging?;
    let ReplaceDstStaging { src, dst, .. } = &staging;
    copy_dir_tree(src, dst)?;
    assert!(dst.join("f.txt").exists());
    Ok(())
}

#[test]
fn copy_dir_tree_creates_missing_destination_parent_chain() -> io::Result<()> {
    let (root, src, dst) = make_src_dst_scaffold()?;
    let dst = dst.join("missing").join("parents");
    fs::create_dir_all(&src)?;
    fs::write(src.join("f.txt"), b"hello")?;

    copy_dir_tree(&src, &dst)?;

    assert_eq!(fs::read(dst.join("f.txt"))?, b"hello");
    drop(root);
    Ok(())
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
) -> io::Result<()> {
    let staging = symlink_in_source_staging?;
    let SymlinkInSourceStaging { src, dst, .. } = &staging;
    #[expect(clippy::expect_used, reason = "the test asserts the copy is rejected")]
    let err = { copy_dir_tree(src, dst).expect_err("failed to copy dir tree") };
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    assert!(
        err.to_string().contains("refusing to follow symlink"),
        "unexpected error message: {err}"
    );
    Ok(())
}

#[test]
#[cfg(unix)]
fn copy_dir_tree_rejects_symlink_as_source_root() -> io::Result<()> {
    use std::os::unix::fs::symlink;

    let root = TempDir::new()?;
    let tree = root.path().join("tree");
    let src = root.path().join("src");
    let dst = root.path().join("dst");
    fs::create_dir_all(&tree)?;
    fs::write(tree.join("f.txt"), b"x")?;
    symlink(&tree, &src)?;
    #[expect(clippy::expect_used, reason = "the test asserts the copy is rejected")]
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
    Ok(())
}

#[test]
#[cfg(unix)]
fn copy_dir_tree_symlink_source_does_not_remove_destination() -> io::Result<()> {
    use std::os::unix::fs::symlink;

    let root = TempDir::new()?;
    let tree = root.path().join("tree");
    let src = root.path().join("src");
    let dst = root.path().join("dst");
    fs::create_dir_all(&tree)?;
    fs::write(tree.join("in-tree.txt"), b"inside-tree")?;
    fs::create_dir_all(&dst)?;
    fs::write(dst.join("marker.txt"), b"untouched")?;
    symlink(&tree, &src)?;
    #[expect(clippy::expect_used, reason = "the test asserts the copy is rejected")]
    let err = copy_dir_tree(&src, &dst).expect_err("symlink source must be rejected");
    assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    assert!(
        err.to_string().contains("refusing to follow symlink"),
        "unexpected error message: {err}"
    );
    assert!(dst.is_dir(), "destination directory must still exist");
    assert_eq!(
        fs::read_to_string(dst.join("marker.txt"))?,
        "untouched",
        "destination contents must be unchanged (remove_destination must not run)"
    );
    Ok(())
}

#[test]
fn copy_dir_tree_rejects_identical_source_and_destination() -> io::Result<()> {
    let root = TempDir::new()?;
    let dir = root.path().join("tree");
    fs::create_dir_all(&dir)?;
    #[expect(clippy::expect_used, reason = "the test asserts the copy is rejected")]
    let err = copy_dir_tree(&dir, &dir).expect_err("identical source and destination");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    assert!(
        err.to_string().contains("refusing overlapping"),
        "unexpected error message: {err}"
    );
    Ok(())
}

#[test]
fn copy_dir_tree_rejects_destination_inside_source() -> io::Result<()> {
    let root = TempDir::new()?;
    let src = root.path().join("src");
    fs::create_dir_all(&src)?;
    fs::write(src.join("f.txt"), b"x")?;
    let dst = src.join("nested_dst");
    #[expect(clippy::expect_used, reason = "the test asserts the copy is rejected")]
    let err = copy_dir_tree(&src, &dst).expect_err("destination inside source");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    assert!(
        err.to_string().contains("refusing overlapping"),
        "unexpected error message: {err}"
    );
    Ok(())
}

#[test]
fn copy_dir_tree_rejects_source_inside_destination() -> io::Result<()> {
    let root = TempDir::new()?;
    let dst = root.path().join("dst");
    fs::create_dir_all(&dst)?;
    let src = dst.join("inner_src");
    fs::create_dir_all(&src)?;
    fs::write(src.join("g.txt"), b"y")?;
    #[expect(clippy::expect_used, reason = "the test asserts the copy is rejected")]
    let err = copy_dir_tree(&src, &dst).expect_err("source inside destination");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    assert!(
        err.to_string().contains("refusing overlapping"),
        "unexpected error message: {err}"
    );
    Ok(())
}
