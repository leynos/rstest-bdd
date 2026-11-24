//! Tests for the Decision D3 path form embedded in generated code.
//!
//! The cases pass a synthetic manifest directory to
//! `manifest_relative_feature_path_in`, so none of them touches the process
//! environment or needs the `#[serial(cargo_manifest_dir)]` lock.

use std::path::Path;

#[cfg(not(windows))]
use super::render_feature_path;
use super::{manifest_relative_feature_path, manifest_relative_feature_path_in};

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

#[test]
fn absolute_path_inside_manifest_becomes_relative() {
    let (manifest, inside, _) = native_absolute_path_fixtures();
    let value = manifest_relative_feature_path_in(Path::new(inside), Path::new(manifest));
    assert_eq!(value, "tests/x.feature");
}

#[test]
fn absolute_path_outside_manifest_stays_absolute() {
    let (manifest, _, outside) = native_absolute_path_fixtures();
    let value = manifest_relative_feature_path_in(Path::new(outside), Path::new(manifest));
    #[cfg(windows)]
    assert_eq!(value, "C:/repo/shared/x.feature");
    #[cfg(not(windows))]
    assert_eq!(value, outside);
}

#[test]
fn external_paths_remain_absolute_after_containment_checks() {
    let (manifest, ..) = native_absolute_path_fixtures();
    #[cfg(windows)]
    let cases = [
        ("../shared/x.feature", "C:/repo/crates/shared/x.feature"),
        (
            r"C:\repo\crates\my\..\shared\x.feature",
            "C:/repo/crates/my/../shared/x.feature",
        ),
    ];
    #[cfg(not(windows))]
    let cases = [
        ("../shared/x.feature", "/repo/crates/shared/x.feature"),
        (
            "/repo/crates/my/../shared/x.feature",
            "/repo/crates/my/../shared/x.feature",
        ),
    ];

    for (input, expected) in cases {
        let value = manifest_relative_feature_path_in(Path::new(input), Path::new(manifest));
        assert_eq!(value, expected);
    }
}
