//! Compile-time tests for GPUI harness macro integration.
//!
//! These tests live with the GPUI harness crate so the core `rstest-bdd`
//! package can be published without resolving GPUI-specific dev-dependencies.
#![cfg(feature = "native-gpui-tests")]

use std::fs;
use std::path::PathBuf;

use rstest_bdd_harness::trybuild_staging::{copy_dir_tree, copy_file};

#[test]
fn gpui_macro_fixtures_compile() -> Result<(), Box<dyn std::error::Error>> {
    stage_trybuild_support_files()?;
    let tests = trybuild::TestCases::new();
    for case in [
        "tests/fixtures_macros/scenario_attributes_gpui.rs",
        "tests/fixtures_macros/scenario_attributes_gpui_absolute.rs",
        "tests/fixtures_macros/scenario_harness_gpui_default.rs",
        "tests/fixtures_macros/scenarios_attributes_gpui.rs",
        "tests/fixtures_macros/scenarios_harness_gpui_default.rs",
    ] {
        tests.pass(case);
    }
    tests.compile_fail("tests/fixtures_macros/scenario_harness_gpui_sync_rejected.rs");
    Ok(())
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
