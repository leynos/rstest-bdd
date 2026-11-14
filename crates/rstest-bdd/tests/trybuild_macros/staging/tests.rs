//! Snapshot-rendering regression tests for trybuild support.

use rstest::rstest;

use super::*;

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

#[rstest]
#[case::feature_suffix(
    "$WORKSPACE/target/tests/trybuild/rstest-bdd/tests/features/x.feature",
    "tests/features/x.feature"
)]
#[case::trybuild_crate_root("$WORKSPACE/target/tests/trybuild/rstest-bdd", "")]
fn outside_snapshot_root_uses_native_trybuild_prefix(#[case] original: &str, #[case] suffix: &str) {
    let target_root = r"C:\outside\target";
    let separator = std::path::MAIN_SEPARATOR;
    let mut expected =
        [target_root, "tests", "trybuild", "rstest-bdd"].join(&separator.to_string());
    if !suffix.is_empty() {
        expected.push(separator);
        expected.push_str(suffix);
    }

    assert_eq!(apply_snapshot_target_root(original, target_root), expected);
}
