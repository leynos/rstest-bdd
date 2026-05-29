//! Compile-time tests for Tokio harness macro integration.
//!
//! These tests live with the Tokio harness crate so the core `rstest-bdd`
//! package can be published without resolving Tokio harness dev-dependencies.

use std::path::PathBuf;

use rstest_bdd_harness::macrotest_support::{
    assert_snapshot_contains, assert_snapshot_omits, snapshot_refresh_is_enabled,
    trybuild_crate_root,
};
use rstest_bdd_harness::trybuild_staging::copy_file;
use serial_test::serial;

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn fixture_path(relative: &str) -> PathBuf {
    crate_root().join(relative)
}

#[test]
#[serial]
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
    if !snapshot_refresh_is_enabled() {
        return;
    }
    for fixture in [
        "tests/fixtures_macros/scenario_harness_tokio_default.rs",
        "tests/fixtures_macros/scenario_harness_tokio_override_default.rs",
        "tests/fixtures_macros/scenarios_harness_tokio_default.rs",
        "tests/fixtures_macros/scenarios_attributes_tokio.rs",
    ] {
        macrotest::expand_without_refresh(fixture);
    }
}

#[test]
fn tokio_snapshots_encode_attribute_boundaries() {
    for relative in [
        "tests/fixtures_macros/scenario_harness_tokio_default.expanded.rs",
        "tests/fixtures_macros/scenario_harness_tokio_override_default.expanded.rs",
        "tests/fixtures_macros/scenarios_harness_tokio_default.expanded.rs",
    ] {
        let path = fixture_path(relative);
        assert_snapshot_contains(&path, &["#[rstest::rstest]", "HarnessAdapter>::run"]);
        assert_snapshot_omits(&path, "tokio::test");
    }
    assert_snapshot_contains(
        &fixture_path("tests/fixtures_macros/scenarios_attributes_tokio.expanded.rs"),
        &["#[tokio::test", "async fn"],
    );
}

fn stage_trybuild_support_files() -> Result<(), Box<dyn std::error::Error>> {
    let crate_root = crate_root();
    let trybuild_root = tokio_trybuild_crate_root()?;
    copy_file(
        &crate_root.join("tests/fixtures_macros/basic.feature"),
        &trybuild_root.join("basic.feature"),
    )?;
    copy_file(
        &crate_root.join("tests/fixtures_macros/scenarios_harness_tokio_default.feature"),
        &trybuild_root.join("scenarios_harness_tokio_default.feature"),
    )?;
    Ok(())
}

fn tokio_trybuild_crate_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
    trybuild_crate_root(&crate_root().join("Cargo.toml"), "rstest-bdd-harness-tokio")
}
