//! Tests for crate-identity normalization and out-directory canonicalization.
//!
//! Registry keys are built from `CARGO_PKG_NAME` plus the compile-time out
//! directory, and two expansions of the same crate must agree on that key
//! even when one path spells the directory relatively, with `.` components,
//! or through a symlink. These cases exercise that agreement directly; the
//! step-validation outcomes live in the parent module.

use camino::Utf8PathBuf;
use rstest::rstest;
use serial_test::serial;
use tempfile::{tempdir, tempdir_in};

use super::{
    super::{
        crate_id::{canonicalize_out_dir, normalize_crate_id},
        *,
    },
    support::{TempWorkingDir, clear_registry, create_dir_all_cap, temp_working_dir},
};

/// Convert a temporary path to UTF-8, failing the test with the offending path.
fn as_utf8(path: std::path::PathBuf) -> Utf8PathBuf {
    Utf8PathBuf::from_path_buf(path)
        .map_err(|path| format!("path should be valid UTF-8: {}", path.display()))
        .expect("test setup should succeed")
}

/// Canonicalize a path the way `normalize_crate_id` expects, keeping the
/// original when the platform cannot resolve it.
fn expected_canonical(path: &Utf8PathBuf) -> Utf8PathBuf {
    path.as_path()
        .canonicalize_utf8()
        .unwrap_or_else(|_| path.clone())
}

#[test]
fn normalizes_crate_id_without_out_dir_component() {
    assert_eq!(normalize_crate_id("my_crate").as_ref(), "my_crate");
}

#[cfg(windows)]
#[test]
fn normalizes_windows_drive_letter_out_dir() {
    let id = normalize_crate_id("demo:C:/a/b");
    assert_eq!(id.as_ref(), "demo:C:/a/b");
}

#[serial]
#[test]
fn normalizes_relative_out_dir_paths() {
    let temp = tempdir_in(".").expect("test setup should succeed");
    let abs = as_utf8(temp.path().to_path_buf());
    let cwd = as_utf8(std::env::current_dir().expect("test setup should succeed"));
    let relative = abs.strip_prefix(&cwd).expect("test setup should succeed");

    let crate_id = format!("demo:./{}", relative.as_str());
    let normalized = normalize_crate_id(&crate_id);

    assert_eq!(
        normalized.as_ref(),
        format!("demo:{}", expected_canonical(&abs).as_str())
    );
}

#[test]
fn leaves_unresolvable_out_dir_paths_unchanged() {
    let temp = tempdir().expect("create temp directory");
    let missing = as_utf8(temp.path().join("missing"));
    let crate_id = format!("demo:{}", missing.as_str());
    assert_eq!(normalize_crate_id(&crate_id).as_ref(), crate_id);
}

#[rstest]
#[serial]
fn canonicalize_out_dir_resolves_relative_components(
    temp_working_dir: std::io::Result<TempWorkingDir>,
) {
    let temp_working_dir = temp_working_dir.expect("test setup should succeed");
    let nested_dir = temp_working_dir.join("nested");
    create_dir_all_cap(nested_dir.as_path()).expect("test setup should succeed");

    let spelled_oddly = temp_working_dir.path().join("nested/.");
    let canonical = canonicalize_out_dir(spelled_oddly.as_path());

    assert_eq!(canonical, expected_canonical(&nested_dir));
    assert!(
        canonical.is_absolute(),
        "canonical path should be absolute: {canonical}"
    );
}

#[cfg(unix)]
#[test]
fn canonicalize_out_dir_resolves_symlinks() {
    let temp = tempdir().expect("create temp directory");
    let base = as_utf8(temp.path().to_path_buf());
    let target = base.join("target");
    create_dir_all_cap(target.as_path()).expect("create target directory for canonicalization");
    let link = base.join("link");
    // `cap_std` has no symlink constructor, so this one call stays with std.
    std::os::unix::fs::symlink(target.as_std_path(), link.as_std_path())
        .expect("create symlink to target");

    assert_eq!(
        canonicalize_out_dir(link.as_path()),
        expected_canonical(&target)
    );
}

#[test]
fn canonicalize_out_dir_returns_original_when_unresolvable() {
    let temp = tempdir().expect("create temp directory");
    let missing = as_utf8(temp.path().join("missing"));
    assert_eq!(canonicalize_out_dir(missing.as_path()), missing);
}

#[serial]
#[test]
fn canonicalizes_equivalent_crate_paths_in_registry() {
    clear_registry();
    let temp = tempdir().expect("test setup should succeed");
    let abs = as_utf8(temp.path().to_path_buf());
    let crate_id = format!("demo:{}", abs.as_str());
    let alt_id = format!("demo:{}/.", abs.as_str());

    register_step_for_crate(StepKeyword::Given, "first pattern", &crate_id);
    register_step_for_crate(StepKeyword::Given, "second pattern", &alt_id);

    let registry = REGISTERED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert_eq!(
        registry.len(),
        1,
        "expected canonical crate IDs to share entry"
    );
    let (stored_id, defs) = registry
        .iter()
        .next()
        .ok_or("expected at least one crate entry")
        .expect("test setup should succeed");
    assert_eq!(stored_id.as_ref(), normalize_crate_id(&crate_id).as_ref());
    assert_eq!(
        defs.patterns(StepKeyword::Given).len(),
        2,
        "expected both patterns to be stored"
    );
    drop(registry);
    clear_registry();
}
