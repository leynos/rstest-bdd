//! Tests for fixture-file write failures and their diagnostic labels.

use std::{
    any::Any,
    panic::{self, AssertUnwindSafe},
};

use camino::Utf8Path;
use cap_std::{ambient_authority, fs::Dir};
use rstest::{fixture, rstest};

use super::write_fixture_file;

/// Provoke a `write_fixture_file` failure and assert its panic keeps the
/// caller's label.
///
/// `set_up` prepares whatever obstruction the failure path needs; `path` and
/// `label` are forwarded to `write_fixture_file` unchanged.
fn assert_write_fixture_file_panic(
    crate_dir: &Dir,
    set_up: impl FnOnce(&Dir),
    path: &Utf8Path,
    label: &str,
    expected_prefix: &str,
) {
    set_up(crate_dir);

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        write_fixture_file(crate_dir, path, b"stderr", label);
    }));
    let message = captured_panic_message(result);

    assert!(
        message.starts_with(expected_prefix),
        "panic message should start with {expected_prefix:?}: {message}",
    );
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

struct FixtureWriteContext {
    _temp_dir: tempfile::TempDir,
    crate_dir: Dir,
}

#[rstest_bdd_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn fixture_write_context() -> FixtureWriteContext {
    let temp_dir = match tempfile::tempdir() {
        Ok(temp_dir) => temp_dir,
        Err(error) => panic!("failed to create temporary directory: {error}"),
    };
    let crate_dir = match Dir::open_ambient_dir(temp_dir.path(), ambient_authority()) {
        Ok(crate_dir) => crate_dir,
        Err(error) => panic!("failed to open temporary directory: {error}"),
    };
    FixtureWriteContext {
        _temp_dir: temp_dir,
        crate_dir,
    }
}

#[rstest]
fn write_fixture_file_preserves_create_directory_panic_label(
    fixture_write_context: FixtureWriteContext,
) {
    assert_write_fixture_file_panic(
        &fixture_write_context.crate_dir,
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

#[rstest]
fn write_fixture_file_preserves_write_panic_label(fixture_write_context: FixtureWriteContext) {
    assert_write_fixture_file_panic(
        &fixture_write_context.crate_dir,
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
