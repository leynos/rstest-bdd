//! Preparation of the Tokio harness trybuild build tree.
//!
//! `tokio_macro_fixtures_compile` drives `trybuild` against a generated crate
//! whose dependency graph includes `rstest-bdd`, `rstest-bdd-macros`, `tokio`
//! and the macro crates behind them. On a cold target directory — a clean
//! checkout, an evicted cache, or the coverage lane's fresh target — that graph
//! is compiled inside the timed test's own deadline, which is the failure mode
//! issue 910 records.
//!
//! The graph is compiled here instead, by the `nested_fixture_preparation`
//! test that `.config/nextest.toml` schedules first with a budget of its own.
//! The timed test gates on the stamp this writes, so a run that skipped the
//! preparation fails immediately naming the preparation command rather than
//! spending its deadline on a cold build.

use std::io;

use rstest_bdd_harness::{macrotest_support::TrybuildTree, nested_cargo::PreparationSummary};

use super::stage_trybuild_support_files;

/// Stable family name for this crate's prepared tree.
const FAMILY: &str = "trybuild-harness-tokio";

/// Subdirectory of the shared trybuild root holding the generated crate.
///
/// `trybuild` names the generated project after the source manifest's package,
/// so this must match [`env!("CARGO_PKG_NAME")`].
const TARGET_SUBDIR: &str = "rstest-bdd-harness-tokio";

/// Compile-pass fixture that gives the generated crate a bin target.
///
/// The generated crate's dependency graph comes from this crate's manifest, not
/// from which fixtures are queued, so one existing compile-pass fixture warms
/// everything the timed run needs and leaves every fixture under test, every
/// `.stderr` snapshot and every `.expanded.rs` snapshot untouched.
const PROBE: &str = "tests/fixtures_macros/scenario_attributes_tokio.rs";

/// The trybuild tree this test binary prepares.
pub(super) fn tree() -> TrybuildTree {
    TrybuildTree::new(
        super::crate_root().as_path(),
        TARGET_SUBDIR,
        FAMILY,
        variant(),
    )
}

/// The generated crate's directory under the shared trybuild root.
pub(super) fn generated_crate_root() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    tree().generated_crate_root()
}

/// The feature selection this binary was built with.
///
/// `trybuild` propagates the running test binary's enabled features to the
/// generated crate, so a different selection is a different build graph. This
/// crate declares no features of its own, but recording the label keeps the
/// stamp honest if one is ever added.
fn variant() -> String { "no-features".to_owned() }

/// Stage support files and compile the probe into the generated crate.
///
/// Staging runs first because the probe's `#[scenario(path = "...")]` resolves
/// its feature file against the generated crate's manifest directory, exactly
/// as the timed test requires.
///
/// This is also the lazy path the timed test takes under plain `cargo test`,
/// so preparation and assertion cannot drift apart.
pub(super) fn prepare() -> io::Result<PreparationSummary> {
    stage_trybuild_support_files().map_err(|error| io::Error::other(error.to_string()))?;
    let tests = trybuild::TestCases::new();
    tests.pass(PROBE);
    drop(tests);
    // `trybuild` streams its nested Cargo's output to this process's own
    // streams, so the unit count cannot be captured here. The preparation log
    // still records the elapsed time, which is the figure the timeout budget is
    // sized from.
    Ok(PreparationSummary::default())
}

/// Compile the generated crate and its dependency graph before the timed cases.
#[test]
#[serial_test::serial]
fn nested_fixture_preparation() {
    tree().prepare_now(prepare).expect(
        "the Tokio harness trybuild tree should prepare; the probe is a compile-pass fixture",
    );
}
