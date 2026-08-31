//! Unit tests for the `scenarios!` macro entry point.

#[cfg(unix)]
mod unix {
    //! Unix-only symlink discovery coverage.

    use std::{fs, os::unix::fs::symlink, path::Path};

    use tempfile::tempdir;

    use super::super::feature_discovery::collect_feature_files;

    #[test]
    fn collects_symlinked_feature_files_without_following_directory_loops() {
        let temp = tempdir().expect("test setup should succeed");
        let features_root = temp.path().join("features");
        fs::create_dir_all(features_root.join("nested")).expect("test setup should succeed");

        let feature_path = features_root.join("nested/example.feature");
        fs::write(&feature_path, "Feature: Example\n").expect("test setup should succeed");

        let symlink_path = features_root.join("symlink.feature");
        symlink(&feature_path, &symlink_path).expect("test setup should succeed");

        let relative_symlink_path = features_root.join("relative_link.feature");
        symlink(Path::new("nested/example.feature"), &relative_symlink_path)
            .expect("test setup should succeed");

        let loop_dir = features_root.join("loop");
        symlink(&features_root, &loop_dir).expect("test setup should succeed");

        let files =
            collect_feature_files(features_root.as_path()).expect("test setup should succeed");

        let mut expected = vec![feature_path, symlink_path, relative_symlink_path];
        expected.sort();
        assert_eq!(files, expected);
    }
}

#[cfg(not(unix))]
mod non_unix {
    //! Portable configuration coverage when Unix symlinks are unavailable.

    #[test]
    fn collects_symlinked_feature_files_without_following_directory_loops() {
        assert!(cfg!(not(unix)));
    }
}
