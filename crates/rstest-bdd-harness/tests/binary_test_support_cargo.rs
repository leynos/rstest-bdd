//! Integration tests that spawn `cargo` via [`rstest_bdd_harness::binary_test_support`].
//!
//! These live outside the library test binary and are omitted on Windows, where
//! nested `cargo` from tests is unreliable under nextest (see `.cargo/nextest.toml`).

#![cfg(not(windows))]

use std::path::Path;

use rstest_bdd_harness::binary_test_support::{
    build_binary, locate_or_build_binary, target_directory_for_manifest,
};

#[test]
fn target_directory_for_invalid_manifest_returns_err() {
    let result = target_directory_for_manifest(Path::new("/nonexistent/Cargo.toml"));
    assert!(
        result.is_err(),
        "expected error for non-existent manifest, got: {result:?}"
    );
}

#[test]
fn target_directory_for_workspace_manifest_returns_ok() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let result = target_directory_for_manifest(&manifest);
    let Ok(target_dir) = result else {
        panic!("expected Ok for valid manifest, got: {result:?}");
    };
    assert!(
        target_dir.ends_with("target")
            || target_dir
                .components()
                .any(|component| component.as_os_str() == "target"),
        "expected target directory to contain a `target` component, got: {}",
        target_dir.display()
    );
}

#[test]
fn build_binary_returns_err_for_nonexistent_workspace() {
    let result = build_binary(Path::new("/nonexistent/workspace"), "nonexistent-binary");
    match result {
        Err(_) => {}
        Ok(output) => assert!(
            !output.status.success(),
            "build_binary should not succeed for a nonexistent workspace"
        ),
    }
}

#[test]
fn build_binary_captures_output_on_failure() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../");
    match build_binary(&workspace_root, "__nonexistent_binary_xyzzy__") {
        Err(_) => {}
        Ok(output) => {
            assert!(
                !output.status.success(),
                "should fail for nonexistent binary"
            );
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                !stderr.is_empty(),
                "expected cargo to emit diagnostic output to stderr"
            );
        }
    }
}

#[test]
fn locate_or_build_binary_returns_err_for_invalid_manifest() {
    let workspace = Path::new("/nonexistent/workspace");
    let manifest = Path::new("/nonexistent/does-not-exist/Cargo.toml");
    let result = locate_or_build_binary(
        manifest,
        workspace,
        "__rstest_bdd_harness_locate_invalid_manifest__",
    );
    assert!(
        result.is_err(),
        "expected error for invalid manifest, got: {result:?}"
    );
}
