//! Regression tests for roadmap item 10.3.3: a `.feature`-only edit must
//! rebuild the scenario binary.
//!
//! The tests are the behavioural specification in
//! `tests/features/rebuild_invalidation.feature`, bound through `#[scenario]`
//! — the feature file's two scenarios *are* the two regression tests. Each
//! scenario's step functions read a `OnceLock`-cached result produced by the
//! shared harness in the `harness` module, so the expensive nested-cargo
//! experiment runs once per process rather than once per step.
//!
//! The two scenarios are deliberately split, and each goes red for its own
//! reason:
//!
//! 1. *A bound feature file is a tracked build dependency* fails before the
//!    fix because the fixture's dep-info lists no `.feature` file (no tracking
//!    item is emitted).
//! 2. *Editing only a feature file forces a rebuild and a fresh failure* fails
//!    before the fix because the second `cargo test` succeeds when it should
//!    fail — the stale binary compiled from the old Gherkin text passes.
//!
//! The dep-info scenario is the cheap, direct-filesystem contract and survives
//! even if the expensive rebuild scenario is ever `#[ignore]`d; see the
//! harness module's `//!` doc for why that split matters.

use googletest::prelude::*;
use rstest_bdd_macros::{given, scenario, then, when};
use serial_test::serial;

#[path = "documentation_examples/mod.rs"]
mod documentation_examples;
#[path = "feature_rebuild_invalidation/harness.rs"]
mod harness;

// ---------------------------------------------------------------------------
// Step functions for scenario 0: "A bound feature file is a tracked build
// dependency".
// ---------------------------------------------------------------------------

#[given("a scenario crate bound to a feature file")]
fn a_bound_crate() {}

#[when("the crate is compiled")]
fn the_crate_is_compiled() {
    let _ = harness::dep_info_outcome();
}

/// Assert the fixture's test-binary dep-info lists the feature file exactly
/// once.
///
/// Exactly-once is a stable contract: rustc and Cargo deduplicate the entry
/// even when a file is included many times, so `count == 1` pins the
/// one-binding-per-file property from Decision D0 and would catch a future
/// refactor that regressed to one binding per scenario.
#[then("the dep-info for the test binary lists the feature file")]
fn dep_info_lists_feature_file() {
    let outcome = harness::dep_info_outcome();
    if let Some(reason) = &outcome.baseline_error {
        panic!(
            "fixture baseline build failed: {reason}\nresolved child environment:\n{}",
            outcome.child_env_detail
        );
    }
    // `assert_that!` (panic mode) rather than `expect_that!`: the step
    // functions run inside a `#[scenario]`-generated `#[rstest]` test, which
    // has no `#[gtest]` context (see Decision M0 in the 10.3.3 execplan).
    assert_that!(
        outcome.dep_info_entry_count,
        eq(1),
        "dep-info for the fixture test binary must list the bound feature file \
         exactly once; rustc dep-info follows\n{}",
        outcome.dep_info_sample
    );
}

// ---------------------------------------------------------------------------
// Step functions for scenario 1: "Editing only a feature file forces a
// rebuild and a fresh failure".
// ---------------------------------------------------------------------------

#[given("a scenario crate bound to a feature file that passes its test")]
fn a_passing_bound_crate() {
    let outcome = harness::rebuild_outcome();
    assert_that!(
        outcome.baseline_passed,
        is_true(),
        "the pre-edit fixture `cargo test` must pass; the experiment is only \
         meaningful when the bound scenario passes before the edit.\nbaseline \
         output:\n{}",
        outcome.baseline_output
    );
}

#[when("only the feature file is edited to change the expectation")]
fn feature_file_edited() {
    // The edit itself happens inside the harness's rebuild experiment, which
    // has already run by the time this step executes (the Given step above
    // initialized it). Deliberately a no-op: the harness owns the mutation so
    // the two recorded runs share one byte-identical environment.
}

#[then("the next test run recompiles the scenario binary")]
fn next_run_recompiles() {
    let outcome = harness::rebuild_outcome();
    assert_that!(
        outcome.second.recompiled,
        is_true(),
        "the second run must recompile the fixture; its stderr should contain \
         a `Compiling rstest-bdd-rebuild-invalidation-fixture` line.\noutput:\n{}",
        outcome.second.output
    );
}

#[then("the test fails against the new expectation")]
fn test_fails_against_new_expectation() {
    let outcome = harness::rebuild_outcome();
    assert_that!(
        outcome.second.failed,
        is_true(),
        "the second run must FAIL — before the fix it succeeds against the \
         stale binary compiled from the old Gherkin text.\noutput:\n{}",
        outcome.second.output
    );
    // The load-bearing proof: the new expectation string exists only in the
    // new Gherkin text, so a failure message naming it proves the binary was
    // recompiled from that text.
    assert_that!(
        outcome.second.names_new_expectation,
        is_true(),
        "the second run must name the new expectation in its output.\noutput:\n{}",
        outcome.second.output
    );
}

// ---------------------------------------------------------------------------
// Step functions for scenario 2: "Adding a feature file to a bound directory
// triggers a rebuild".
// ---------------------------------------------------------------------------

#[given("a scenario crate whose build script tracks its feature directory")]
fn build_script_tracks_feature_directory() {
    // Runs the whole addition experiment: writes the `build.rs` extracted
    // from the documented `scenarios-build-script` example into the
    // `feature_addition` fixture, adds the `build` key, and confirms the
    // baseline scenario runs.
    let outcome = harness::addition_outcome();
    if let Some(reason) = &outcome.baseline_error {
        panic!("build-script fixture setup failed: {reason}");
    }
    // The baseline scenario must have run before the addition; the
    // experiment's second run is only meaningful then.
    assert_that!(
        outcome
            .baseline_output
            .contains("baseline_the_baseline_scenario"),
        is_true(),
        "the pre-addition `cargo test` must run the fixture's baseline \
         scenario.\noutput:\n{}",
        outcome.baseline_output
    );
}

#[when("a new feature file is added to that directory")]
fn new_feature_file_added() {
    // The addition itself happens inside the harness's addition experiment,
    // which has already run by the time this step executes (the Given step
    // above initialized it). Deliberately a no-op, mirroring the edit step of
    // the rebuild scenario.
}

#[then("the next test run recompiles and runs the new scenario")]
fn next_run_recompiles_and_runs_new_scenario() {
    let outcome = harness::addition_outcome();
    assert_that!(
        outcome.second_run_recompiled,
        is_true(),
        "the second run must recompile the fixture after the directory changes.\noutput:\n{}",
        outcome.second_run_output
    );
    // The contract: the added scenario's generated test runs. `Compiling`
    // alone is only corroboration; a scenario that actually ran is proof the
    // new Gherkin text was discovered.
    assert_that!(
        outcome.new_scenario_ran,
        is_true(),
        "the second run must execute the test generated from the added \
         `.feature` file (its name contains `zzz_added_the_added_scenario`).\noutput:\n{}",
        outcome.second_run_output
    );
}

// ---------------------------------------------------------------------------
// The two regression tests, the `scenarios!` directory case, and the
// file-addition case.
// ---------------------------------------------------------------------------

#[scenario("tests/features/rebuild_invalidation.feature", index = 0)]
#[serial]
fn bound_feature_file_is_tracked_dependency() {}

#[scenario("tests/features/rebuild_invalidation.feature", index = 1)]
#[serial]
fn feature_edit_forces_rebuild_and_fresh_failure() {}

#[scenario("tests/features/rebuild_invalidation.feature", index = 2)]
#[serial]
fn feature_addition_triggers_rebuild() {}

/// A `scenarios!` directory with a `tags =` filter that excludes every
/// scenario in one file must still track that file: it was parsed, so an
/// edit to it must trigger a rebuild even though no test is generated from
/// it. Plain `#[test]` (not scenario-bound) because the behavioural spec has
/// exactly two scenarios.
#[test]
#[serial]
fn scenarios_directory_filtered_file_is_tracked() {
    let outcome = harness::dep_info_outcome();
    if let Some(reason) = &outcome.baseline_error {
        panic!(
            "fixture baseline build failed: {reason}\nresolved child environment:\n{}",
            outcome.child_env_detail
        );
    }
    assert_that!(
        outcome.scenarios_no_match_tracked,
        is_true(),
        "the filtered .feature file must still appear in the dep-info \
         primary rule; rustc dep-info follows\n{}",
        outcome.dep_info_sample
    );
}
