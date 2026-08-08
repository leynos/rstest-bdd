//! Unit tests for normalizer and path helpers used by trybuild macro tests.
use std::{
    any::Any,
    panic::{self, AssertUnwindSafe},
};

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs::Dir};

use super::{
    wrappers::{FixtureStderr, FixtureTestPath},
    *,
};

fn write_fixture_file(crate_dir: &Dir, path: &Utf8Path, bytes: &[u8], label: &str) {
    if let Some(parent) = path.parent() {
        if let Err(error) = crate_dir.create_dir_all(parent.as_std_path()) {
            panic!("failed to create directory for {label}: {error}");
        }
    }
    if let Err(error) = crate_dir.write(path.as_std_path(), bytes) {
        panic!("failed to write {label}: {error}");
    }
}

fn captured_panic_message(result: Result<(), Box<dyn Any + Send>>) -> String {
    let Err(payload) = result else {
        panic!("expected helper to panic");
    };
    if let Some(message) = payload.downcast_ref::<&str>() {
        return (*message).to_owned();
    }
    let Some(message) = payload.downcast_ref::<String>() else {
        panic!("expected helper panic payload to be a string");
    };
    message.clone()
}

struct NormalizerFixture {
    expected_path: Utf8PathBuf,
    actual_path: Utf8PathBuf,
}

impl NormalizerFixture {
    fn new(
        test_path: FixtureTestPath<'_>,
        expected: FixtureStderr<'_>,
        actual: FixtureStderr<'_>,
    ) -> Self {
        let fixture_path = Utf8Path::new(test_path.as_ref());
        let crate_dir = match Dir::open_ambient_dir(
            Utf8Path::new(env!("CARGO_MANIFEST_DIR")),
            ambient_authority(),
        ) {
            Ok(crate_dir) => crate_dir,
            Err(error) => panic!("failed to open crate directory: {error}"),
        };

        let expected_path = expected_stderr_path(fixture_path.as_std_path());
        write_fixture_file(
            &crate_dir,
            &expected_path,
            expected.as_ref().as_bytes(),
            "expected stderr fixture",
        );

        let actual_path = wip_stderr_path(fixture_path.as_std_path());
        write_fixture_file(
            &crate_dir,
            &actual_path,
            actual.as_ref().as_bytes(),
            "wip stderr fixture",
        );

        Self {
            expected_path,
            actual_path,
        }
    }
}

impl Drop for NormalizerFixture {
    fn drop(&mut self) {
        if let Ok(crate_dir) = Dir::open_ambient_dir(
            Utf8Path::new(env!("CARGO_MANIFEST_DIR")),
            ambient_authority(),
        ) {
            drop(crate_dir.remove_file(self.expected_path.as_std_path()));
            drop(crate_dir.remove_file(self.actual_path.as_std_path()));
        }
    }
}

#[test]
fn wip_stderr_path_builds_target_location() {
    let path =
        wip_stderr_path(Utf8Path::new("tests/fixtures_macros/__helper_case.rs").as_std_path());
    assert_eq!(path, Utf8Path::new("target/tests/wip/__helper_case.stderr"));
}

#[test]
#[should_panic(expected = "trybuild test path must include file name")]
fn wip_stderr_path_panics_without_file_name() { wip_stderr_path(Utf8Path::new("").as_std_path()); }

#[test]
fn expected_stderr_path_replaces_extension() {
    let path = expected_stderr_path(Utf8Path::new("tests/ui_macros/example.output").as_std_path());
    assert_eq!(path, Utf8Path::new("tests/ui_macros/example.stderr"));
}

#[test]
fn expected_stderr_path_handles_multiple_extensions() {
    let path =
        expected_stderr_path(Utf8Path::new("tests/ui_macros/example.feature.rs").as_std_path());
    assert_eq!(
        path,
        Utf8Path::new("tests/ui_macros/example.feature.stderr")
    );
}

/// Stage a temporary directory, provoke a `write_fixture_file` failure, and
/// assert the resulting panic message keeps the caller's label.
///
/// `set_up` prepares whatever obstruction the failure path needs; `path` and
/// `label` are forwarded to `write_fixture_file` unchanged.
fn assert_write_fixture_file_panic(
    set_up: impl FnOnce(&Dir),
    path: &Utf8Path,
    label: &str,
    expected_prefix: &str,
) {
    let temp_dir = match tempfile::tempdir() {
        Ok(temp_dir) => temp_dir,
        Err(error) => panic!("failed to create temporary directory: {error}"),
    };
    let crate_dir = match Dir::open_ambient_dir(temp_dir.path(), ambient_authority()) {
        Ok(crate_dir) => crate_dir,
        Err(error) => panic!("failed to open temporary directory: {error}"),
    };
    set_up(&crate_dir);

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        write_fixture_file(&crate_dir, path, b"stderr", label);
    }));
    let message = captured_panic_message(result);

    assert!(
        message.starts_with(expected_prefix),
        "panic message should start with {expected_prefix:?}: {message}",
    );
}

#[test]
fn write_fixture_file_preserves_create_directory_panic_label() {
    assert_write_fixture_file_panic(
        |crate_dir| {
            if let Err(error) = crate_dir.write("blocked", b"not a directory") {
                panic!("failed to create blocked path: {error}");
            }
        },
        Utf8Path::new("blocked/output.stderr"),
        "expected stderr fixture",
        "failed to create directory for expected stderr fixture:",
    );
}

#[test]
fn write_fixture_file_preserves_write_panic_label() {
    assert_write_fixture_file_panic(
        |crate_dir| {
            if let Err(error) = crate_dir.create_dir("blocked") {
                panic!("failed to create blocked directory: {error}");
            }
        },
        Utf8Path::new("blocked"),
        "wip stderr fixture",
        "failed to write wip stderr fixture:",
    );
}

mod normalizers;
