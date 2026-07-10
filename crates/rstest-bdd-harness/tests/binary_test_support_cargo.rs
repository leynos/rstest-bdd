//! Integration tests that spawn `cargo` via [`rstest_bdd_harness::binary_test_support`].
//!
//! Unix-only: nested `cargo` from tests is unreliable on Windows under nextest
//! (see `.cargo/nextest.toml`).
//!
//! `temp_env` scopes `CARGO_BIN_EXE_*` removal so we stay within the workspace
//! `forbid(unsafe_code)` policy (Edition 2024 makes `remove_var` unsafe).

#![cfg(unix)]

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rstest_bdd_harness::binary_test_support::{
    BinaryLocateError, BinaryName, build_binary, locate_or_build_binary,
    target_directory_for_manifest,
};

/// Directory under [`std::env::temp_dir()`] guaranteed absent before the test runs.
fn unique_absent_temp_dir(label: &str) -> std::io::Result<PathBuf> {
    // A clock before the epoch degrades to zero nanoseconds; the process id
    // and label still keep the path unique enough for test scratch space.
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let dir =
        std::env::temp_dir().join(format!("rstest_bdd_{label}_{}_{nanos}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
    }
    assert!(!dir.exists(), "temp dir should be absent before test");
    Ok(dir)
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

#[test]
fn target_directory_for_invalid_manifest_returns_err() -> std::io::Result<()> {
    let manifest = unique_absent_temp_dir("missing_manifest")?.join("Cargo.toml");
    assert!(
        !manifest.exists(),
        "test setup: manifest path must not exist: {}",
        manifest.display()
    );
    let result = target_directory_for_manifest(&manifest);
    assert!(
        result.is_err(),
        "expected error for non-existent manifest, got: {result:?}"
    );
    Ok(())
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
fn build_binary_returns_err_for_nonexistent_workspace() -> std::io::Result<()> {
    let workspace = unique_absent_temp_dir("no_workspace")?;
    let result = build_binary(&workspace, BinaryName::new("nonexistent-binary"));
    assert!(
        result.is_err(),
        "expected build_binary to fail when the workspace directory does not exist, got: {result:?}"
    );
    Ok(())
}

#[expect(
    clippy::expect_used,
    reason = "integration-style tests panic if cargo cannot be spawned for the workspace root"
)]
#[test]
fn build_binary_captures_output_on_failure() {
    let workspace_root = workspace_root();
    let output = build_binary(
        &workspace_root,
        BinaryName::new("__nonexistent_binary_xyzzy__"),
    )
    .expect("should spawn cargo for an existing workspace");
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

#[test]
fn locate_or_build_binary_returns_err_for_invalid_manifest() -> std::io::Result<()> {
    let workspace = unique_absent_temp_dir("locate_invalid_workspace")?;
    let manifest = unique_absent_temp_dir("locate_invalid_manifest")?.join("Cargo.toml");
    assert!(
        !manifest.exists(),
        "test setup: manifest path must not exist: {}",
        manifest.display()
    );
    let result = locate_or_build_binary(
        &manifest,
        &workspace,
        BinaryName::new("__rstest_bdd_harness_locate_invalid_manifest__"),
    );
    assert!(
        result.is_err(),
        "expected error for invalid manifest, got: {result:?}"
    );
    Ok(())
}

#[expect(
    clippy::expect_used,
    reason = "integration-style tests panic on improbable locate or cargo metadata failures"
)]
#[test]
fn locate_or_build_reports_build_failed_for_nonexistent_binary() {
    let root = workspace_root();
    let name = BinaryName::new("__nonexistent_binary_xyzzy__");
    temp_env::with_var_unset(format!("CARGO_BIN_EXE_{name}"), || {
        let err = locate_or_build_binary(&root.join("Cargo.toml"), &root, name)
            .expect_err("expected a build failure for a nonexistent binary");
        match err {
            BinaryLocateError::BuildFailed(_capture) => {}
            other => panic!("expected BuildFailed, got: {other:?}"),
        }
    });
}

#[expect(
    clippy::expect_used,
    reason = "integration-style tests panic on improbable locate or spawn failures"
)]
#[test]
fn locate_or_build_returns_command_for_workspace_binary() {
    let root = workspace_root();
    let name = BinaryName::new("todo-cli");
    temp_env::with_var_unset(format!("CARGO_BIN_EXE_{name}"), || {
        let mut cmd = locate_or_build_binary(&root.join("Cargo.toml"), &root, name)
            .expect("should locate or build a workspace binary");

        let program = cmd.get_program().to_owned();
        let program_path = Path::new(&program);

        assert!(
            program_path.is_file(),
            "expected resolved program path to exist and be a file: {program:?}"
        );

        let expected_name = format!("{}{}", name.as_str(), std::env::consts::EXE_SUFFIX);
        let actual_name = program_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        assert_eq!(
            actual_name, expected_name,
            "expected program file name {expected_name}, got {actual_name}"
        );

        let status = cmd.arg("--help").status().expect("spawn --help");
        assert!(
            status.success(),
            "expected `todo-cli --help` to exit successfully, got {status:?}"
        );
    });
}
