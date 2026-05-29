//! Compile-time tests for GPUI harness macro integration.
//!
//! These tests live with the GPUI harness crate so the core `rstest-bdd`
//! package can be published without resolving GPUI-specific dev-dependencies.
#![cfg(feature = "native-gpui-tests")]

use std::fs;
use std::path::PathBuf;

use rstest_bdd_harness::macrotest_support::{
    assert_snapshot_contains, assert_snapshot_omits, snapshot_refresh_is_enabled,
    trybuild_crate_root,
};
use rstest_bdd_harness::trybuild_staging::{copy_dir_tree, copy_file};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn fixture_path(relative: &str) -> PathBuf {
    crate_root().join(relative)
}

#[test]
fn gpui_macro_fixtures_compile() -> Result<(), Box<dyn std::error::Error>> {
    stage_trybuild_support_files()?;
    let tests = trybuild::TestCases::new();
    for case in [
        "tests/fixtures_macros/scenario_attributes_gpui.rs",
        "tests/fixtures_macros/scenario_attributes_gpui_absolute.rs",
        "tests/fixtures_macros/scenario_harness_gpui_default.rs",
        "tests/fixtures_macros/scenario_harness_gpui_override_default.rs",
        "tests/fixtures_macros/scenarios_attributes_gpui.rs",
        "tests/fixtures_macros/scenarios_harness_gpui_default.rs",
        "tests/fixtures_macros/scenarios_harness_gpui_override_default.rs",
    ] {
        tests.pass(case);
    }
    tests.compile_fail("tests/fixtures_macros/scenario_harness_gpui_sync_rejected.rs");
    Ok(())
}

#[test]
fn gpui_macro_expansions_match_snapshots() {
    if !snapshot_refresh_is_enabled() {
        return;
    }
    for fixture in [
        "tests/fixtures_macros/scenario_harness_gpui_default.rs",
        "tests/fixtures_macros/scenario_harness_gpui_override_default.rs",
        "tests/fixtures_macros/scenarios_harness_gpui_default.rs",
        "tests/fixtures_macros/scenarios_harness_gpui_override_default.rs",
    ] {
        macrotest::expand_without_refresh(fixture);
    }
}

#[test]
fn gpui_snapshots_encode_attribute_boundaries() {
    for relative in [
        "tests/fixtures_macros/scenario_harness_gpui_default.expanded.rs",
        "tests/fixtures_macros/scenario_harness_gpui_override_default.expanded.rs",
        "tests/fixtures_macros/scenarios_harness_gpui_default.expanded.rs",
        "tests/fixtures_macros/scenarios_harness_gpui_override_default.expanded.rs",
    ] {
        let path = fixture_path(relative);
        assert_snapshot_contains(&path, &["#[rstest::rstest]", "HarnessAdapter>::run"]);
        assert_snapshot_omits(&path, "gpui::test");
    }
}

fn stage_trybuild_support_files() -> Result<(), Box<dyn std::error::Error>> {
    let crate_root = crate_root();
    let trybuild_root = gpui_trybuild_crate_root()?;
    copy_file(
        &crate_root.join("tests/fixtures_macros/basic.feature"),
        &trybuild_root.join("basic.feature"),
    )?;
    fs::create_dir_all(trybuild_root.join("tests/features"))?;
    copy_dir_tree(
        &crate_root.join("tests/features/auto"),
        &trybuild_root.join("tests/features/auto"),
    )?;
    Ok(())
}

fn gpui_trybuild_crate_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
    trybuild_crate_root(&crate_root().join("Cargo.toml"), "rstest-bdd-harness-gpui")
}
