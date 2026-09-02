//! Target-directory and snapshot-rendering regression tests for trybuild support.

use rstest::rstest;

use super::*;

#[rstest]
#[case::default_target(
    "/workspace/target/debug/deps/trybuild_macros-a1b2c3",
    "/workspace/target"
)]
#[case::coverage_target(
    "/workspace/target/llvm-cov-target/debug/deps/trybuild_macros-a1b2c3",
    "/workspace/target/llvm-cov-target"
)]
fn derives_target_root_from_test_executable(#[case] executable: &str, #[case] expected: &str) {
    assert_eq!(
        target_directory_from_test_executable(Utf8Path::new(executable)),
        Some(Utf8PathBuf::from(expected))
    );
}

#[test]
#[serial_test::serial(trybuild_target_directory)]
fn target_directory_uses_running_test_executable() {
    let workspace_root = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Utf8Path::parent)
        .expect("workspace root must be two levels above the manifest directory");
    let expected = target_directory_from_running_test_executable()
        .expect("integration tests should run from Cargo's `deps` directory");

    assert_eq!(trybuild_target_directory(workspace_root), expected);
}

#[rstest]
#[case::workspace_target("/workspace/target", "/workspace", "$WORKSPACE/target")]
#[case::coverage_target(
    "/workspace/target/llvm-cov-target",
    "/workspace",
    "$WORKSPACE/target/llvm-cov-target"
)]
#[case::windows_separators(
    r"C:\workspace\target\llvm-cov-target",
    r"C:\workspace",
    "$WORKSPACE/target/llvm-cov-target"
)]
#[case::outside_workspace("/shared/target", "/workspace", "/shared/target")]
#[case::outside_workspace_with_windows_separators(
    r"C:\outside\target",
    r"D:\workspace",
    r"C:\outside\target"
)]
fn renders_snapshot_target_root(
    #[case] target_root: &str,
    #[case] workspace_root: &str,
    #[case] expected: &str,
) {
    assert_eq!(
        snapshot_target_root(Utf8Path::new(target_root), Utf8Path::new(workspace_root)),
        expected
    );
}
