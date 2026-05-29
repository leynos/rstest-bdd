//! Compile-time tests for GPUI harness macro integration.
//!
//! These tests live with the GPUI harness crate so the core `rstest-bdd`
//! package can be published without resolving GPUI-specific dev-dependencies.
#![cfg(feature = "native-gpui-tests")]

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use rstest_bdd_harness::trybuild_staging::{copy_dir_tree, copy_file};

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
    if !macrotest_snapshot_refresh_is_enabled() {
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
    for path in [
        "tests/fixtures_macros/scenario_harness_gpui_default.expanded.rs",
        "tests/fixtures_macros/scenario_harness_gpui_override_default.expanded.rs",
        "tests/fixtures_macros/scenarios_harness_gpui_default.expanded.rs",
        "tests/fixtures_macros/scenarios_harness_gpui_override_default.expanded.rs",
    ] {
        assert_snapshot_contains(path, &["#[rstest::rstest]", "HarnessAdapter>::run"]);
        assert_snapshot_omits(path, "gpui::test");
    }
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
    let trybuild_root = trybuild_crate_root()?;
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

fn trybuild_crate_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
        .no_deps()
        .exec()?;
    Ok(metadata
        .target_directory
        .into_std_path_buf()
        .join("tests/trybuild/rstest-bdd-harness-gpui"))
}
