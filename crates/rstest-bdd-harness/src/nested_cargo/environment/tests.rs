//! The target-directory resolution's failure path.
//!
//! The success paths are covered where they are used, through
//! [`NestedCargoEnv`]; what cannot be reached from those is a working
//! directory that cannot be read. No portable fixture removes the
//! directory the test process is running in, so the lookup is injected.

use std::{ffi::OsStr, io, path::PathBuf};

use super::absolute_target_directory_with;

/// An absolute path in the form this target actually uses.
///
/// A Unix literal is *not* absolute on Windows: `Path::is_absolute` needs a
/// prefix there, so `/abs/target` takes the relative branch and the test
/// asserted a fact about another platform. Naming the drive here keeps one
/// fixture that is genuinely absolute on both, rather than two cases that
/// would each only exercise one platform.
fn absolute_fixture() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(r"C:\abs\target")
    } else {
        PathBuf::from("/abs/target")
    }
}

#[test]
fn an_absolute_value_never_consults_the_working_directory() {
    // The lookup is not merely unused here, it must not be called: an
    // absolute path is already resolved, and a process whose working
    // directory has been deleted still names a perfectly good absolute
    // target. Panicking in the lookup states that rather than asserting
    // the result and hoping.
    let absolute = absolute_fixture();
    let resolved = absolute_target_directory_with(absolute.as_os_str(), || {
        panic!("an absolute target directory must not consult the working directory")
    })
    .expect("an absolute path is resolved without any lookup");
    assert_eq!(resolved, absolute);
}

#[test]
fn an_unreadable_working_directory_is_reported_rather_than_left_relative() {
    // The regression: this used to fall back to the relative path, which
    // then named one directory for this process and another for a child
    // running in a fixture directory. The error must name the value and
    // the cause, because the caller has no other way to learn that the
    // working directory is what went wrong.
    let error = absolute_target_directory_with(OsStr::new("relative-cache"), || {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "the working directory is gone",
        ))
    })
    .expect_err("an unreadable working directory must not yield a path");

    assert_eq!(
        error.kind(),
        io::ErrorKind::NotFound,
        "the cause's kind must survive: {error}"
    );
    let message = error.to_string();
    assert!(
        message.contains("relative-cache"),
        "the unresolved value must be named: {message}"
    );
    assert!(
        message.contains("the working directory is gone"),
        "the underlying cause must be named: {message}"
    );
}

#[test]
fn a_relative_value_is_joined_to_the_read_working_directory() {
    let resolved = absolute_target_directory_with(OsStr::new("relative-cache"), || {
        Ok(PathBuf::from("/some/cwd"))
    })
    .expect("a readable working directory resolves a relative value");
    assert_eq!(resolved, PathBuf::from("/some/cwd/relative-cache"));
}

/// A drive-relative value resolves against its own drive, not the lookup.
///
/// `C:cache` carries a prefix and no root, so it is not absolute and
/// `join` leaves it unchanged: joining replaces the base whenever the
/// argument has a prefix of its own. The result would then be the same
/// relative value this helper promises never to return, naming one
/// directory for the test and another for the child. The lookup is made to
/// panic because a value that already names a drive has no business
/// consulting the process's directory.
#[cfg(windows)]
#[test]
fn a_drive_relative_value_is_resolved_against_its_drive() {
    let resolved = absolute_target_directory_with(OsStr::new(r"C:cache"), || {
        panic!("a drive-relative path must not consult the working directory")
    })
    .expect("a drive-relative path resolves against its drive root");
    assert!(
        resolved.is_absolute(),
        "the resolved path must be absolute: {}",
        resolved.display()
    );
    assert_eq!(resolved, PathBuf::from(r"C:\cache"));
}

/// A rooted Unix path is not mistaken for a drive-relative one.
///
/// The drive branch must be inert on every path the platform does not
/// classify as prefix-relative. `/abs/target` is the shape that a careless
/// `starts_with`-style test would misclassify on Unix, so it is kept here
/// as the control beside the Windows case.
#[cfg(not(windows))]
#[test]
fn a_rooted_unix_path_is_left_to_the_absolute_branch() {
    let resolved = absolute_target_directory_with(OsStr::new("/abs/target"), || {
        panic!("an absolute path must not consult the working directory")
    })
    .expect("an absolute path resolves without a lookup");
    assert_eq!(resolved, PathBuf::from("/abs/target"));
}
