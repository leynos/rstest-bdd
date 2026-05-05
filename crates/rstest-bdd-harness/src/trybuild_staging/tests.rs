//! Unit tests for [`super::copy_file`] and [`super::copy_dir_tree`] staging helpers.

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
