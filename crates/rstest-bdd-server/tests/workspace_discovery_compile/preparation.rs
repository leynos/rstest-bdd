//! Preparation of the server crate's trybuild build tree.
//!
//! `find_feature_files_requires_result_handling` drives `trybuild` against a
//! generated crate whose dependency graph includes `rstest-bdd-server` and its
//! own dependency closure (`async-lsp`, `tokio`, `tower`, `gherkin`, `syn` and
//! the rest). On a cold target directory that graph is compiled inside the
//! test's own deadline, which is the failure mode issue 910 records for the
//! other trybuild suites.
//!
//! The graph is compiled here instead, by the `nested_fixture_preparation`
//! test that `.config/nextest.toml` schedules first with a budget of its own.
//! The timed test gates on the stamp this writes, so a run that skipped the
//! preparation fails immediately naming the preparation command rather than
//! spending its deadline on a cold build.
//!
//! This binary holds exactly two tests, and both reach the shared preparation
//! lock before running `trybuild`, so it needs no `#[serial]` attribute of its
//! own: `preparation::ensure_prepared` serializes the lazy path, and
//! `trybuild` itself takes a file lock on the generated project directory.

use std::{io, path::PathBuf};

use rstest_bdd_harness::{
    macrotest_support::{TrybuildTree, reject_trybuild_filter},
    nested_cargo::PreparationSummary,
};

/// Stable family name for this crate's prepared tree.
const FAMILY: &str = "trybuild-server-workspace-discovery";

/// Subdirectory of the shared trybuild root holding the generated crate.
///
/// `trybuild` names the generated project after the source manifest's package,
/// so this must match [`env!("CARGO_PKG_NAME")`].
const TARGET_SUBDIR: &str = "rstest-bdd-server";

/// Compile-pass fixture that gives the generated crate a bin target.
///
/// The generated crate's dependency graph comes from this crate's manifest, not
/// from which fixtures are queued, so the existing compile-pass fixture warms
/// everything a build-mode timed run needs.
const PROBE: &str = "tests/fixtures_workspace_discovery/find_feature_files_result.rs";

/// The compile-fail fixture, probed only to warm check mode.
///
/// `trybuild` builds when any queued case is a pass case and checks otherwise.
/// An unfiltered timed run queues both fixtures and so builds, which the pass
/// probe covers. A run filtered to `trybuild=find_feature_files_vec.rs` queues
/// this fixture alone, selects `cargo check`, and reads the metadata graph the
/// pass probe never wrote: a valid stamp admits it before `prepare` can refuse
/// the filter, and the timed case then checks cold dependencies.
///
/// `compile_fail` still requires the fixture to fail and still compares the
/// `.stderr` snapshot beside it, so a drifted snapshot fails the preparation
/// rather than warming nothing unnoticed.
const COMPILE_FAIL_PROBE: &str = "tests/fixtures_workspace_discovery/find_feature_files_vec.rs";

/// The crate root whose trybuild tree is prepared.
fn crate_root() -> PathBuf { PathBuf::from(env!("CARGO_MANIFEST_DIR")) }

/// The trybuild tree this test binary prepares.
pub(crate) fn tree() -> TrybuildTree {
    TrybuildTree::new(crate_root().as_path(), TARGET_SUBDIR, FAMILY, variant())
}

/// The feature selection this binary was built with.
///
/// `trybuild` propagates the running test binary's enabled features to the
/// generated crate, so a different selection is a different build graph. This
/// crate's `test-support` feature only adds a dev-facing dependency and is not
/// enabled by any lane that runs this test binary, but recording the resolved
/// selection keeps the stamp honest if that changes.
fn variant() -> String {
    if cfg!(feature = "test-support") {
        "test-support".to_owned()
    } else {
        "no-features".to_owned()
    }
}

/// Compile the generated crate and its dependency graph.
///
/// This is also the lazy path the timed test takes under plain `cargo test`,
/// so preparation and assertion cannot drift apart.
///
/// Unlike the Tokio and GPUI harness trees, this crate's fixtures reference no
/// feature file, so there are no support files to stage and `trybuild` panics
/// rather than returning an error. The `io::Result` is the shape
/// `TrybuildTree::prepare_now` and `ensure_prepared` take, and it is real: a
/// run carrying a `trybuild=` fixture filter is refused here, because such a
/// run would drop one or both probes, warm part of the graph at most, and
/// still write the complete stamp.
pub(crate) fn prepare() -> io::Result<PreparationSummary> {
    reject_trybuild_filter()?;
    // Two `TestCases`, not one: a single object holding both probes selects
    // build mode for both and leaves check mode cold. The Tokio and GPUI trees
    // warm both modes the same way.
    let pass_probe = trybuild::TestCases::new();
    pass_probe.pass(PROBE);
    drop(pass_probe);
    let compile_fail_probe = trybuild::TestCases::new();
    compile_fail_probe.compile_fail(COMPILE_FAIL_PROBE);
    drop(compile_fail_probe);
    // `trybuild` streams its nested Cargo's output to this process's own
    // streams, so the unit count cannot be captured here. The preparation log
    // still records the elapsed time, which is the figure the timeout budget is
    // sized from.
    Ok(PreparationSummary::default())
}

/// Compile the generated crate and its dependency graph before the timed case.
#[test]
fn nested_fixture_preparation() {
    tree()
        .prepare_now(prepare)
        .expect("the server trybuild tree should prepare; both probes are committed fixtures");
}
