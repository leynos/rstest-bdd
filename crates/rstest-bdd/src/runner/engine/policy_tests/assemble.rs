//! LEM-2, INV-13: the terminal record and the fold.
//!
//! `assemble` stores what the driver observed and derives nothing the fold can
//! derive. A skip in particular stores **no** failure — forced or not — because
//! [`ScenarioOutcome::into_harness_result`](crate::runner::ScenarioOutcome::into_harness_result)
//! decides that, and two sources of truth for it would eventually disagree.

use super::fixtures::*;
use crate::{
    StepKeyword,
    runner::{
        ScenarioFailure,
        ScenarioStatus,
        engine::policy::{Terminal, assemble},
        outcome::{StepOutcome, StepStatus},
        source::SourceLocation,
        test_invocation,
    },
};

// --- assemble: the terminal shapes --------------------------------------

/// A permitted skip stores the skip record and **no** failure.
///
/// This is the shape D16 could not express. `ScenarioFailure::ForcedSkip` is
/// derived at fold time, so storing one here as well would be a second source
/// of truth; the fold decides, and `is_passed` deliberately does not.
#[test]
fn a_permitted_skip_stores_no_failure() {
    let outcome = assemble(
        vec![skipped(0), bypassed(1)],
        Some(skip_terminal(Some("waiting on upstream"))),
        PERMITTED,
    );

    assert_eq!(outcome.status(), ScenarioStatus::Skipped);
    assert!(outcome.failure().is_none(), "a skip is not a failure");
    let record = outcome.skip().expect("a skip record is stored");
    assert_eq!(record.at(), 0);
    assert_eq!(record.message(), Some("waiting on upstream"));
    assert!(record.allow_skipped(), "the plan permitted this skip");
    assert!(!record.forced_failure());
    assert_eq!(outcome.into_harness_result(), Ok(()));
}

/// A refused skip also stores no failure — the fold derives it.
#[test]
fn a_refused_skip_stores_no_failure_but_folds_to_one() {
    let outcome = assemble(vec![skipped(0)], Some(skip_terminal(None)), REFUSED);

    assert_eq!(outcome.status(), ScenarioStatus::Skipped);
    assert!(
        outcome.failure().is_none(),
        "the failure is derived by the fold, never stored",
    );
    let record = outcome.skip().expect("a skip record is stored");
    assert!(record.forced_failure());
    assert!(!record.allow_skipped(), "the effective flag is recorded");

    let Err(failure) = outcome.into_harness_result() else {
        panic!("a refused skip must fail the suite");
    };
    assert!(matches!(failure, ScenarioFailure::ForcedSkip(_)));
}

/// The recorded `allow_skipped` is the *effective* one, so D10's invariant
/// `forced_failure == !allow_skipped && fail_on_skipped` holds of the record.
///
/// Without the `|| !fail_on_skipped` term a plan-level `allow_skipped = false`
/// with a permissive scope would record `allow_skipped: false` and
/// `forced_failure: false`, breaking the identity the policy rows assert.
#[test]
fn a_permissive_scope_records_an_effective_allow_skipped() {
    let outcome = assemble(vec![skipped(0)], Some(skip_terminal(None)), PERMISSIVE);

    let record = outcome.skip().expect("a skip record is stored");
    assert!(
        record.allow_skipped(),
        "the effective flag folds in fail_on_skipped = false",
    );
    assert!(!record.forced_failure());
    assert_eq!(outcome.into_harness_result(), Ok(()));
}

/// A skip's source comes from the `Terminal`, not from `details` by index.
///
/// That is what makes `status: Skipped` and `skip: Some(_)` inseparable: one
/// match arm produces both, so no index can be out of range.
#[test]
fn the_skip_records_the_terminal_source() {
    let outcome = assemble(vec![skipped(0)], Some(skip_terminal(None)), PERMITTED);

    let record = outcome.skip().expect("a skip record is stored");
    assert_eq!(
        record.source().map(SourceLocation::line),
        Some(12),
        "the skipping step's line is recorded",
    );
}

#[test]
fn a_failure_stores_the_step_failure() {
    let error = not_found();
    let outcome = assemble(
        vec![failed(0), bypassed(1)],
        Some(Terminal::Fail {
            index: 0,
            error: error.clone(),
        }),
        PERMITTED,
    );

    assert_eq!(outcome.status(), ScenarioStatus::Failed);
    assert!(outcome.skip().is_none());
    let Some(ScenarioFailure::Step {
        index,
        error: carried,
    }) = outcome.failure()
    else {
        panic!(
            "a failed run stores a step failure, got {:?}",
            outcome.failure()
        );
    };
    assert_eq!(*index, 0);
    assert_eq!(carried, &error, "the error is carried verbatim");
}

/// A full pass has neither a skip nor a failure.
#[test]
fn a_complete_run_has_no_terminal_event() {
    let outcome = assemble(vec![passed(0), passed(1)], None, PERMITTED);

    assert_eq!(outcome.status(), ScenarioStatus::Passed);
    assert!(outcome.skip().is_none());
    assert!(outcome.failure().is_none());
    assert_eq!(outcome.into_harness_result(), Ok(()));
}

/// Every planned invocation is recorded, and everything after the terminal is
/// `Bypassed`. INV-2's structural half, checkable here without a registry.
#[test]
fn assemble_preserves_the_record_it_was_handed() {
    let outcome = assemble(
        vec![skipped(0), bypassed(1), bypassed(2)],
        Some(skip_terminal(None)),
        PERMITTED,
    );

    assert_eq!(outcome.steps().len(), 3);
    assert_eq!(
        outcome.steps().first().map(StepOutcome::status),
        Some(StepStatus::Skipped)
    );
    let statuses = outcome
        .steps()
        .iter()
        .map(StepOutcome::status)
        .collect::<Vec<_>>();
    assert_eq!(
        statuses,
        vec![
            StepStatus::Skipped,
            StepStatus::Bypassed,
            StepStatus::Bypassed
        ],
    );
}

/// The empty plan is INV-13's case, and it is *not* an assembly decision.
///
/// `assemble` reports `Passed` because running nothing is not itself an error;
/// the fold is what rejects it. Pinning this here stops a later "fix" from
/// moving the empty-plan rule into assembly, where `is_passed` would then
/// disagree with it.
#[test]
fn an_empty_plan_is_passed_but_folds_to_a_failure() {
    let outcome = assemble(Vec::new(), None, PERMITTED);

    assert_eq!(outcome.status(), ScenarioStatus::Passed);
    let Err(failure) = outcome.into_harness_result() else {
        panic!("an empty plan must not fold to a clean pass");
    };
    assert!(matches!(failure, ScenarioFailure::EmptyPlan));
}

/// The insertion fate reaches the outcome untouched.
///
/// `assemble` does not recompute it — the driver recorded what `insert_value`
/// returned. A value that matched no fixture must still be visible as
/// `NoMatch`, because that is the only signal for the case the runtime
/// otherwise leaves silent (INV-12).
#[test]
fn a_recorded_insertion_fate_is_preserved() {
    let fate = crate::runner::ValueFate::NoMatch;
    let step = StepOutcome::passed(
        0,
        &test_invocation(StepKeyword::Given, "a calculator", Some(&location())),
        Some(fate),
    );
    let outcome = assemble(vec![step], None, PERMITTED);

    assert_eq!(
        outcome
            .steps()
            .first()
            .and_then(StepOutcome::value_insertion),
        Some(fate)
    );
}
