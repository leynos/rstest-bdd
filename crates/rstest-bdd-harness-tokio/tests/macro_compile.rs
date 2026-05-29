//! Compile-time tests for Tokio harness macro integration.
//!
//! These tests live with the Tokio harness crate so the core `rstest-bdd`
//! package can be published without resolving Tokio harness dev-dependencies.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use rstest_bdd_harness::trybuild_staging::copy_file;

#[test]
fn tokio_macro_fixtures_compile() -> Result<(), Box<dyn std::error::Error>> {
    stage_trybuild_support_files()?;
    let tests = trybuild::TestCases::new();
    for case in [
        "tests/fixtures_macros/scenario_attributes_tokio.rs",
        "tests/fixtures_macros/scenario_attributes_tokio_sync.rs",
        "tests/fixtures_macros/scenario_attributes_tokio_dedup.rs",
        "tests/fixtures_macros/scenario_harness_tokio_default.rs",
        "tests/fixtures_macros/scenario_harness_tokio_override_default.rs",
        "tests/fixtures_macros/scenarios_attributes_tokio.rs",
        "tests/fixtures_macros/scenarios_harness_tokio_default.rs",
    ] {
        tests.pass(case);
    }
    for case in [
        "tests/fixtures_macros/scenario_default_policy_async_no_harness_rejected.rs",
        "tests/fixtures_macros/scenario_harness_tokio_async_rejected.rs",
        "tests/fixtures_macros/scenarios_runtime_alias_deprecated.rs",
    ] {
        tests.compile_fail(case);
    }
    Ok(())
}

#[test]
fn tokio_macro_expansions_match_snapshots() {
    if !macrotest_snapshot_refresh_is_enabled() {
        return;
    }
    macrotest::expand_without_refresh("tests/fixtures_macros/scenario_harness_tokio_default.rs");
    macrotest::expand_without_refresh(
        "tests/fixtures_macros/scenario_harness_tokio_override_default.rs",
    );
    macrotest::expand_without_refresh("tests/fixtures_macros/scenarios_harness_tokio_default.rs");
    macrotest::expand_without_refresh("tests/fixtures_macros/scenarios_attributes_tokio.rs");
}

#[test]
fn tokio_snapshots_encode_attribute_boundaries() {
    for path in [
        "tests/fixtures_macros/scenario_harness_tokio_default.expanded.rs",
        "tests/fixtures_macros/scenario_harness_tokio_override_default.expanded.rs",
        "tests/fixtures_macros/scenarios_harness_tokio_default.expanded.rs",
    ] {
        assert_snapshot_contains(path, &["#[rstest::rstest]", "HarnessAdapter>::run"]);
        assert_snapshot_omits(path, "tokio::test");
    }
    assert_snapshot_contains(
        "tests/fixtures_macros/scenarios_attributes_tokio.expanded.rs",
        &["#[tokio::test", "async fn"],
    );
}

fn macrotest_snapshot_refresh_is_enabled() -> bool {
    std::env::var_os("RSTEST_BDD_RUN_MACROTEST").is_some() && cargo_expand_is_available()
}

fn cargo_expand_is_available() -> bool {
    Command::new("cargo")
        .args(["expand", "--version"])
        .output()
        .is_ok_and(|output| output.status.success())
}

fn assert_snapshot_contains(path: &str, needles: &[&str]) {
    let contents = read_snapshot(path);
    for needle in needles {
        assert!(
            contents.contains(needle),
            "expected {path} to contain {needle:?}"
        );
    }
}

fn assert_snapshot_omits(path: &str, needle: &str) {
    let contents = read_snapshot(path);
    assert!(
        !contents.contains(needle),
        "expected {path} to omit {needle:?}"
    );
}

fn read_snapshot(path: &str) -> String {
    fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path))
        .unwrap_or_else(|err| panic!("failed to read snapshot {path}: {err}"))
}

fn stage_trybuild_support_files() -> Result<(), Box<dyn std::error::Error>> {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    copy_file(
        &crate_root.join("tests/fixtures_macros/basic.feature"),
        &trybuild_crate_root()?.join("basic.feature"),
    )?;
    copy_file(
        &crate_root.join("tests/fixtures_macros/scenarios_harness_tokio_default.feature"),
        &trybuild_crate_root()?.join("scenarios_harness_tokio_default.feature"),
    )?;
    Ok(())
}

fn trybuild_crate_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
        .no_deps()
        .exec()?;
    Ok(metadata
        .target_directory
        .into_std_path_buf()
        .join("tests/trybuild/rstest-bdd-harness-tokio"))
}
