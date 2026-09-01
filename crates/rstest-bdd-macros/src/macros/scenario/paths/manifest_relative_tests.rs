//! Tests for the Decision D3 path form embedded in generated code.
//!
//! These tests mutate `CARGO_MANIFEST_DIR`, so they must not run concurrently
//! with the canonical-path tests that read it; the shared
//! `#[serial(cargo_manifest_dir)]` lock provides that in-process exclusion.

use std::path::Path;

use serial_test::serial;

use super::manifest_relative_feature_path;
#[cfg(not(windows))]
use super::render_feature_path;

// POSIX `/...` paths are rooted but not absolute on Windows, so use the host's
// native absolute syntax to exercise the absolute-path branch.
#[cfg(not(windows))]
fn native_absolute_path_fixtures() -> (&'static str, &'static str, &'static str) {
    (
        "/repo/crates/my",
        "/repo/crates/my/tests/x.feature",
        "/repo/shared/x.feature",
    )
}

#[cfg(windows)]
fn native_absolute_path_fixtures() -> (&'static str, &'static str, &'static str) {
    (
        r"C:\repo\crates\my",
        r"C:\repo\crates\my\tests\x.feature",
        r"C:\repo\shared\x.feature",
    )
}

#[serial(cargo_manifest_dir)]
#[test]
fn relative_input_passes_through_unchanged() {
    let value = manifest_relative_feature_path(Path::new("tests/features/x.feature"));
    assert_eq!(value, "tests/features/x.feature");
}

#[cfg(not(windows))]
#[test]
fn render_feature_path_preserves_unix_backslashes() {
    let value = render_feature_path(Path::new(r"tests\x.feature"));
    assert_eq!(value, r"tests\x.feature");
}

#[serial(cargo_manifest_dir)]
#[test]
fn absolute_path_inside_manifest_becomes_relative() {
    let (manifest, inside, _) = native_absolute_path_fixtures();
    temp_env::with_var("CARGO_MANIFEST_DIR", Some(manifest), || {
        let value = manifest_relative_feature_path(Path::new(inside));
        assert_eq!(value, "tests/x.feature");
    });
}

#[serial(cargo_manifest_dir)]
#[test]
fn absolute_path_outside_manifest_stays_absolute() {
    let (manifest, _, outside) = native_absolute_path_fixtures();
    temp_env::with_var("CARGO_MANIFEST_DIR", Some(manifest), || {
        let value = manifest_relative_feature_path(Path::new(outside));
        #[cfg(windows)]
        assert_eq!(value, "C:/repo/shared/x.feature");
        #[cfg(not(windows))]
        assert_eq!(value, outside);
    });
}
