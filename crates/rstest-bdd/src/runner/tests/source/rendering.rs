//! INV-7's presentation half: what a caller reads when it renders an outcome.
//!
//! Fidelity is the *data* claim — the parent proves every
//! `StepOutcome::source()` equals the `SourceLocation` the plan supplied — and
//! this is the *presentation* claim. They are separate files because they are
//! separate obligations that a naive implementation satisfies together and a
//! careless one satisfies separately: the accessors can be right while the
//! rendering is wrong, and that is exactly the state this suite found.
//!
//! # Why the snapshot is of `Display` and not `Debug`
//!
//! `Display` is the contract a frontend renders; `Debug` is not a contract, and
//! its churn would train a reviewer to accept snapshot updates reflexively. The
//! distinction matters because a snapshot is the one thing in the suite that
//! fails for a *wording* change, and a snapshot that fails for harmless reasons
//! stops being read.

use super::support::{DECOY_PATH, PROSE_PATH, SPEC_PATH, at, decoy_error, honest_error, line};
use crate::{
    StepKeyword,
    runner::{
        FailureSite,
        ScenarioFailure,
        ScenarioOutcome,
        ScenarioSkip,
        ScenarioStatus,
        SourceLocation,
        StepOutcome,
        test_invocation,
    },
};

/// The `Display` projection, snapshotted across the outcome variants (INV-7).
///
/// # Why one snapshot rather than one per variant
///
/// The variants share a shape — `scenario <status> at step N: <detail>` — and
/// the interesting failure is a variant whose rendering *drifts from that
/// shape*. Separate snapshots would each have to be updated in such a change
/// and could be updated one at a time, which is how a divergence gets
/// committed. One snapshot holding every line makes the parallel structure
/// visible in the diff, so a change that flattened one variant and not the
/// others reads as an asymmetry rather than as unrelated edits.
///
/// The paths are the non-`.feature` ones the support module defines, so the
/// rendering is exercised against identifiers no `.feature` parser could
/// produce, and the message text is unlike the other cases' so a `Display` that
/// printed the wrong variant's payload renders a visibly different line.
///
/// The failure case uses [`honest_error`] rather than the decoy, because this
/// test freezes the *form* of the projection and a decoy path would freeze a
/// value production never emits. The decoy's work is done next door.
#[test]
fn the_display_projection_renders_every_variant() {
    let cases: Vec<(&str, ScenarioOutcome)> = vec![
        (
            "passed",
            ScenarioOutcome::new(
                ScenarioStatus::Passed,
                vec![StepOutcome::passed(
                    0,
                    &test_invocation(
                        StepKeyword::Given,
                        "a calculator",
                        Some(&at(PROSE_PATH, line(1))),
                    ),
                    None,
                )],
                None,
                None,
            ),
        ),
        (
            "skipped, with a message",
            ScenarioOutcome::new(
                ScenarioStatus::Skipped,
                Vec::new(),
                Some(ScenarioSkip::new(
                    0,
                    Some("no database in this lane".to_owned()),
                    Some(at(PROSE_PATH, 31)),
                    false,
                    false,
                )),
                None,
            ),
        ),
        (
            "skipped, without a message",
            ScenarioOutcome::new(
                ScenarioStatus::Skipped,
                Vec::new(),
                Some(ScenarioSkip::new(2, None, None, true, false)),
                None,
            ),
        ),
        (
            "failed at a step",
            ScenarioOutcome::new(
                ScenarioStatus::Failed,
                vec![StepOutcome::failed(
                    0,
                    &test_invocation(
                        StepKeyword::Given,
                        "an undefined step",
                        Some(&at(SPEC_PATH, 7)),
                    ),
                    honest_error(0, SPEC_PATH),
                )],
                None,
                Some(ScenarioFailure::Step {
                    index: 0,
                    error: honest_error(0, SPEC_PATH),
                }),
            ),
        ),
        (
            "failed, the plan empty",
            ScenarioOutcome::new(
                ScenarioStatus::Failed,
                Vec::new(),
                None,
                Some(ScenarioFailure::EmptyPlan),
            ),
        ),
        (
            "failed, a forced skip",
            ScenarioOutcome::new(
                ScenarioStatus::Failed,
                Vec::new(),
                Some(ScenarioSkip::new(
                    1,
                    None,
                    Some(at(SPEC_PATH, 9)),
                    false,
                    true,
                )),
                Some(ScenarioFailure::ForcedSkip(ScenarioSkip::new(
                    1, None, None, false, true,
                ))),
            ),
        ),
    ];

    let rendered: String = cases
        .iter()
        .map(|(label, outcome)| format!("{label}:\n  {outcome}"))
        .collect::<Vec<_>>()
        .join("\n");

    insta::assert_snapshot!(rendered);
}

/// A failed outcome's **rendering** takes its path from the error, not the plan.
///
/// This test exists because writing the snapshot above found it, and it records
/// a real, load-bearing wart rather than a documentation nit.
///
/// INV-7 says "no source is read back out of `ExecutionError`", and the
/// *accessors* honour that: `StepOutcome::source` and
/// `ScenarioOutcome::terminal_source` both return the plan's `SourceLocation`,
/// and the parent proves it. But `ScenarioOutcome`'s `Display` renders a
/// failure through the error's own message, and that message embeds
/// `ExecutionError::feature_path` — a `String` the plan's path was *flattened
/// into* when the runner built the error. A frontend that prints `{outcome}`
/// rather than walking `steps()` therefore gets the error's copy, and the two
/// are equal only because the runner populates the field from the plan.
///
/// The test pins the **disagreement** rather than asserting the two agree,
/// because asserting agreement would be asserting a coincidence: the decoy is
/// what production never produces, and the point is that the rendering has no
/// independent reason to carry the plan's location. Fixing it means rendering
/// the failure from `terminal_source()` rather than from the error's `Display`,
/// which changes a user-visible string and so is a deliberate change rather
/// than a drive-by one. Until then this test is the record: a caller that wants
/// the plan's location must read the accessors, and `Display`'s own doc comment
/// should say so.
///
/// The `FailureSite` assertion is the control — without it, a run that lost its
/// failure entirely would satisfy both string assertions vacuously.
#[test]
fn the_rendered_failure_takes_its_path_from_the_error_not_the_plan() {
    let record = StepOutcome::failed(
        0,
        &test_invocation(
            StepKeyword::Given,
            "an undefined step",
            Some(&at(SPEC_PATH, 7)),
        ),
        decoy_error(0),
    );
    let outcome = ScenarioOutcome::new(
        ScenarioStatus::Failed,
        vec![record],
        None,
        Some(ScenarioFailure::Step {
            index: 0,
            error: decoy_error(0),
        }),
    );

    assert_eq!(
        outcome.failure().map(ScenarioFailure::site),
        Some(FailureSite::Step(0)),
        "the control: the failure is present and sited, so the rendering below is of a real \
         failure rather than of an outcome that lost one",
    );
    assert_eq!(
        outcome.terminal_source().map(SourceLocation::path),
        Some(SPEC_PATH),
        "the accessor returns the plan's path, as INV-7 requires",
    );

    let rendered = format!("{outcome}");
    assert!(
        rendered.contains(DECOY_PATH),
        "the rendering carries the error's own path instead — `{rendered}`",
    );
    assert!(
        !rendered.contains(SPEC_PATH),
        "and it does not carry the plan's path at all, which is the divergence this test records \
         — `{rendered}`",
    );
}
