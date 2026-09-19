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
        SkipPolicyRecord,
        SkipRecord,
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
///
/// The bodies live in [`projections`] rather than inline so each variant reads
/// as one labelled case rather than as a paragraph of constructor arguments;
/// the table itself is what this test is about, and the cases are the payload.
#[test]
fn the_display_projection_renders_every_variant() {
    let rendered: String = projections()
        .iter()
        .map(|(label, outcome)| format!("{label}:\n  {outcome}"))
        .collect::<Vec<_>>()
        .join("\n");

    insta::assert_snapshot!(rendered);
}

/// One labelled outcome per `Display` branch, in the order the snapshot shows.
///
/// The skip is the one outcome carrying a policy *pair*, and it is built here
/// with both halves spelled out — an outcome whose `forced_failure` were
/// recomputed from `allow_skipped` at render time would still render the same
/// line, which is why the conversion test next door asserts the pair rather
/// than this one.
fn projections() -> Vec<(&'static str, ScenarioOutcome)> {
    let mut cases = vec![("passed", passed_case())];
    cases.extend(skip_cases());
    cases.extend([
        ("failed at a step", failed_step_case()),
        ("failed, the plan empty", empty_plan_case()),
        ("failed, a forced skip", forced_skip_case()),
    ]);
    cases
}

/// A clean pass over one step.
fn passed_case() -> ScenarioOutcome {
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
    )
}

/// The two skip renderings, over one skip record each.
///
/// One function rather than two because the pair differs only in what the
/// invocation supplied: both are a `Skipped` outcome with no steps and no
/// failure, so a second body would restate the first and could drift from it.
/// What the pair is *for* is the contrast — a rendering that always printed a
/// colon and an empty detail would look correct for the first row alone, and
/// the second is what says it is not.
fn skip_cases() -> Vec<(&'static str, ScenarioOutcome)> {
    let rows = [
        (
            "skipped, with a message",
            Some("no database in this lane".to_owned()),
            Some(at(PROSE_PATH, 31)),
            0,
            (false, false),
        ),
        ("skipped, without a message", None, None, 2, (true, false)),
    ];

    rows.into_iter()
        .map(
            |(label, message, source, at_index, (allow_skipped, forced_failure))| {
                let skip = ScenarioSkip::new(
                    at_index,
                    SkipRecord { message, source },
                    SkipPolicyRecord {
                        allow_skipped,
                        forced_failure,
                    },
                );
                (
                    label,
                    ScenarioOutcome::new(ScenarioStatus::Skipped, Vec::new(), Some(skip), None),
                )
            },
        )
        .collect()
}

/// A failure at a step, sited by the plan rather than by the error.
fn failed_step_case() -> ScenarioOutcome {
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
    )
}

/// A failure with no step at all, which the fold rejects as an empty plan.
fn empty_plan_case() -> ScenarioOutcome {
    ScenarioOutcome::new(
        ScenarioStatus::Failed,
        Vec::new(),
        None,
        Some(ScenarioFailure::EmptyPlan),
    )
}

/// A skip the policy forces to fail: the one variant carrying two records.
///
/// The skip and the failure hold *separately built* records, because that is
/// the state a caller can be handed — nothing in the type system ties the
/// forced-skip failure to the skip beside it.
fn forced_skip_case() -> ScenarioOutcome {
    ScenarioOutcome::new(
        ScenarioStatus::Failed,
        Vec::new(),
        Some(ScenarioSkip::new(
            1,
            SkipRecord {
                message: None,
                source: Some(at(SPEC_PATH, 9)),
            },
            SkipPolicyRecord {
                allow_skipped: false,
                forced_failure: true,
            },
        )),
        Some(ScenarioFailure::ForcedSkip(ScenarioSkip::new(
            1,
            SkipRecord {
                message: None,
                source: None,
            },
            SkipPolicyRecord {
                allow_skipped: false,
                forced_failure: true,
            },
        ))),
    )
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
