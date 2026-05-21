//! Compile-time tests for Tokio harness macro integration.
//!
//! These tests live with the Tokio harness crate so the core `rstest-bdd`
//! package can be published without resolving Tokio harness dev-dependencies.

use std::path::PathBuf;

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
        "tests/fixtures_macros/scenario_harness_tokio_async_rejected.rs",
        "tests/fixtures_macros/scenarios_runtime_alias_deprecated.rs",
    ] {
        tests.compile_fail(case);
    }
    Ok(())
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
