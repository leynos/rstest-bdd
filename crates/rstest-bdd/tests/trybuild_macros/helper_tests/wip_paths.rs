//! Portable contracts for locating trybuild work-in-progress diagnostics.

use tempfile::tempdir;

use super::{super::read_wip_stderr, Dir, Utf8Path, ambient_authority, write_fixture_file};

#[test]
fn read_wip_stderr_prefers_current_directory() -> std::io::Result<()> {
    let current_tempdir = tempdir()?;
    let fallback_tempdir = tempdir()?;
    let current_dir = Dir::open_ambient_dir(current_tempdir.path(), ambient_authority())?;
    let fallback_dir = Dir::open_ambient_dir(fallback_tempdir.path(), ambient_authority())?;
    let path = Utf8Path::new("wip/observed_windows.stderr");

    write_fixture_file(
        &current_dir,
        path,
        b"current directory output",
        "current wip fixture",
    );
    write_fixture_file(
        &fallback_dir,
        path,
        b"fallback output",
        "fallback wip fixture",
    );

    let (actual, is_in_current_dir) =
        read_wip_stderr(&current_dir, &fallback_dir, path.as_std_path())?;

    assert!(is_in_current_dir);
    assert_eq!(actual, "current directory output");
    Ok(())
}

#[test]
fn read_wip_stderr_falls_back_to_manifest_directory() -> std::io::Result<()> {
    let current_tempdir = tempdir()?;
    let fallback_tempdir = tempdir()?;
    let current_dir = Dir::open_ambient_dir(current_tempdir.path(), ambient_authority())?;
    let fallback_dir = Dir::open_ambient_dir(fallback_tempdir.path(), ambient_authority())?;
    let path = Utf8Path::new("wip/observed_windows.stderr");

    write_fixture_file(
        &fallback_dir,
        path,
        b"fallback output",
        "fallback wip fixture",
    );

    let (actual, is_in_current_dir) =
        read_wip_stderr(&current_dir, &fallback_dir, path.as_std_path())?;

    assert!(!is_in_current_dir);
    assert_eq!(actual, "fallback output");
    Ok(())
}
