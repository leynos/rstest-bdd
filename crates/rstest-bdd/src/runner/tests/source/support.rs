//! The fixtures INV-7's tests share: sources, errors, and status rows.
//!
//! Split out of the parent so both halves of INV-7 — the data claim in
//! `mod.rs` and the presentation claim in `rendering.rs` — can be stated
//! without restating the setup. A duplicated helper would be worse than
//! duplicated prose here: the two halves must agree on what a "decoy path" is,
//! and that agreement is what makes the agreement test in `rendering.rs` mean
//! anything.
//!
//! # The decoy
//!
//! [`ExecutionError`] carries its own `feature_path` string, populated from the
//! plan's source. Reading it back would be the obvious shortcut and it is the
//! one INV-7 forbids, so the fixtures here let the two disagree: an outcome can
//! be built whose *sources* are `spec/cases.toml` and `notes/example.md` while
//! its errors carry [`DECOY_PATH`]. An implementation that read the location
//! out of the error fails every path assertion in the parent — and the
//! rendering test in `rendering.rs` is what closes the same hole for the
//! `Display` projection, which re-renders through `terminal_source()` and so
//! carries the plan's path too. The decoy stays in the fixtures precisely
//! because agreement is only worth asserting when the two could be seen to
//! differ.

use crate::{
    StepKeyword,
    execution::ExecutionError,
    runner::{SourceLocation, StepOutcome, StepStatus, test_invocation},
};

/// The `feature_path` every decoy error carries.
///
/// Deliberately *not* the source any assertion names: an implementation that
/// read the location out of the error rather than out of the plan would pass
/// every assertion in the parent if the two agreed, and fail all of them if
/// they do not.
pub(super) const DECOY_PATH: &str = "tests/features/decoy.feature";

/// The structured-spec path: one of the two sources the tests use.
///
/// Neither path is a `.feature` file, and that is the point. A test written
/// against `features/x.feature` would pass against an implementation that
/// reconstructed the path from a convention rather than carrying the plan's;
/// these two cannot, because no code path in the crate produces them.
///
/// The names are ordinary identifiers because the INV-11 scan in
/// `tests/surface.rs` reads this file and rejects a frontend *token* on any
/// line that is not a comment — including inside a string literal and
/// including an identifier. A fixture that names a frontend therefore has to
/// do it in prose, which is where the reasoning belongs anyway.
pub(super) const SPEC_PATH: &str = "spec/cases.toml";
/// The prose-document path, used where a second identity is needed.
pub(super) const PROSE_PATH: &str = "notes/example.md";

/// A step-not-found error whose `feature_path` is the decoy.
pub(super) fn decoy_error(index: usize) -> ExecutionError { step_not_found(index, DECOY_PATH) }

/// A step-not-found error whose `feature_path` agrees with the plan's source.
///
/// This is what the runner actually builds, so it is the shape the snapshot
/// freezes: the rendering test is about the *form* of the projection, and a
/// decoy path there would freeze a value production never produces.
pub(super) fn honest_error(index: usize, path: &str) -> ExecutionError {
    step_not_found(index, path)
}

/// A `StepNotFound` naming `path` as the feature it was looked for in.
fn step_not_found(index: usize, path: &str) -> ExecutionError {
    ExecutionError::StepNotFound {
        index,
        keyword: StepKeyword::Given,
        text: "an undefined step".to_owned(),
        feature_path: path.to_owned(),
        scenario_name: "demo".to_owned(),
    }
}

/// A one-based line that is never the source line of a scenario.
///
/// Offset far above any line a fixture here uses, so a record that copied the
/// scenario's line — or renumbered from zero — reports a value no assertion
/// expects rather than one that happens to coincide.
pub(super) fn line(step: u32) -> u32 { 100 + step }

/// A `SourceLocation` on `path` at `line`, with no column.
pub(super) fn at(path: &'static str, line: u32) -> SourceLocation {
    SourceLocation::new_static(path, line, None)
}

/// The four statuses a step record can carry, with a record for each.
///
/// Built synthetically rather than by running a scenario: the claim is about
/// what an outcome *carries*, and running one would only add a registry
/// dependency to a statement that needs none. The index, keyword, and text
/// differ per row so that a record which copied one row's fields into another
/// is visible rather than masked by four identical records.
pub(super) fn every_status() -> Vec<(StepStatus, StepOutcome)> {
    let source = at(PROSE_PATH, line(1));
    vec![
        (
            StepStatus::Passed,
            StepOutcome::passed(
                0,
                &test_invocation(StepKeyword::Given, "a passed step", Some(&source)),
                None,
            ),
        ),
        (
            StepStatus::Skipped,
            StepOutcome::skipped(
                1,
                &test_invocation(StepKeyword::When, "a skipped step", Some(&source)),
                Some("asked to skip".to_owned()),
            ),
        ),
        (
            StepStatus::Failed,
            StepOutcome::failed(
                2,
                &test_invocation(StepKeyword::Then, "a failed step", Some(&source)),
                decoy_error(2),
            ),
        ),
        (
            StepStatus::Bypassed,
            StepOutcome::bypassed(
                3,
                &test_invocation(StepKeyword::Then, "a bypassed step", Some(&source)),
            ),
        ),
    ]
}
