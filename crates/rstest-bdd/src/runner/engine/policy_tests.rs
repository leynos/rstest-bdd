//! LEM-1: the decision layer is pure, total, and enumerable.
//!
//! Every test here drives `classify` or `assemble` directly, with no registry
//! and no `StepContext`. That is the point of the split: the stop decision is
//! single-sourced, so the two drivers cannot disagree about it, and the whole
//! of it is checkable without running a scenario.
//!
//! The domain is finite and small. `execute_step` returns
//! `Result<Option<Box<dyn Any>>, ExecutionError>`, and for decision purposes
//! the `Err` side collapses to four classes: `Skip`, `StepNotFound`,
//! `MissingFixtures`, and `HandlerFailed`. `FailureKind` is deliberately not
//! part of the decision, so `HandlerFailed`'s sub-kinds are not distinct
//! inputs here.
//!
//! # What these tests can and cannot establish
//!
//! They establish that `classify` is total over the enumerated classes and
//! that each class maps to the decision D18 records. They establish nothing
//! about whether the drivers *call* it correctly — that is INV-1's and INV-5's
//! job, discharged by the sequence property tests. Keeping the two apart is
//! deliberate: a test driving a real scenario could fail for a driver bug and
//! be misread as a `classify` bug.

use std::sync::Arc;

use crate::{
    StepError, StepKeyword,
    execution::ExecutionError,
    runner::{
        ScenarioFailure, ScenarioStatus,
        engine::policy::{
            Absorbed, SkipPolicy, StepDecision, Terminal, absorb, assemble, classify,
        },
        outcome::{StepOutcome, StepStatus},
        source::SourceLocation,
    },
};

/// The source location every synthetic record is given.
fn location() -> SourceLocation {
    SourceLocation::new_static("notes/demo.md", 12, None)
}

/// A `StepNotFound` failure; the cheapest non-skip `Err` to build.
fn not_found() -> ExecutionError {
    ExecutionError::StepNotFound {
        index: 0,
        keyword: StepKeyword::Given,
        text: "an undefined step".to_owned(),
        feature_path: "notes/demo.md".to_owned(),
        scenario_name: "demo".to_owned(),
    }
}

/// A `MissingFixtures` failure, the second non-skip class.
fn missing_fixtures() -> ExecutionError {
    ExecutionError::MissingFixtures(Arc::new(crate::execution::MissingFixturesDetails {
        step_pattern: "a calculator".to_owned(),
        step_location: "steps.rs:1".to_owned(),
        required: vec!["calc"],
        missing: vec!["calc"],
        missing_requirements: Vec::new(),
        available: Vec::new(),
        has_suggestion: false,
        feature_path: "notes/demo.md".to_owned(),
        scenario_name: "demo".to_owned(),
    }))
}

/// A `HandlerFailed` failure, the third non-skip class.
fn handler_failed() -> ExecutionError {
    ExecutionError::HandlerFailed {
        index: 0,
        keyword: StepKeyword::Given,
        text: "a calculator".to_owned(),
        error: Arc::new(StepError::ExecutionError {
            pattern: "a calculator".to_owned(),
            function: "a_calculator".to_owned(),
            message: "boom".to_owned(),
        }),
        feature_path: "notes/demo.md".to_owned(),
        scenario_name: "demo".to_owned(),
    }
}

/// A skip error carrying the supplied reason.
fn skip(message: Option<&str>) -> ExecutionError {
    ExecutionError::Skip {
        message: message.map(str::to_owned),
    }
}

/// A passing record at `index`.
fn passed(index: usize) -> StepOutcome {
    StepOutcome::passed(
        index,
        StepKeyword::Given,
        "a calculator",
        Some(&location()),
        None,
    )
}

/// A bypassed record at `index`.
fn bypassed(index: usize) -> StepOutcome {
    StepOutcome::bypassed(index, StepKeyword::Given, "a calculator", Some(&location()))
}

/// A skipped record at `index`.
fn skipped(index: usize) -> StepOutcome {
    StepOutcome::skipped(
        index,
        StepKeyword::Given,
        "a pending step",
        Some(&location()),
        Some("waiting on upstream".to_owned()),
    )
}

/// A failed record at `index`.
fn failed(index: usize) -> StepOutcome {
    StepOutcome::failed(
        index,
        StepKeyword::Given,
        "an undefined step",
        Some(&location()),
        not_found(),
    )
}

/// A policy under which a scenario-level skip is permitted.
///
/// Derived through [`SkipPolicy::resolve`] rather than written as a struct
/// literal. The fields are private to the engine, so a literal is possible
/// here, and that is exactly the hazard: a literal spells a *pre*-resolution
/// shape (`allow_skipped: false, fail_on_skipped: false` reads as "permissive"
/// but is not what a resolved permissive policy holds), and a test that
/// constructs one asserts post-resolution invariants against a value the
/// production code never builds. Resolving keeps these constants honest by
/// construction, and states the plan-side flag each one stands for.
const PERMITTED: SkipPolicy = SkipPolicy::resolve(true, true);

/// A policy under which a scenario-level skip is refused.
const REFUSED: SkipPolicy = SkipPolicy::resolve(false, true);

/// A scope-level policy that permits skipping even when the plan does not.
///
/// The plan refuses skipping and the scope overrides it, so the effective
/// `allow_skipped` is `true` — the case the literal got wrong.
const PERMISSIVE: SkipPolicy = SkipPolicy::resolve(false, false);

/// A terminal skip at index 0, as a driver would build it.
fn skip_terminal(message: Option<&str>) -> Terminal {
    Terminal::Skip {
        index: 0,
        message: message.map(str::to_owned),
        source: Some(location()),
    }
}

// --- absorb: insertion happens before classification --------------------

/// A returned value is inserted through the supplied closure and its fate is
/// recorded, whatever the fate was.
///
/// `NoMatch` is the case that matters: it is the only signal for a value that
/// reached no later step, and the runtime emits no warning for it. A run that
/// dropped the fate here would leave a renamed fixture silently green.
#[test]
fn a_returned_value_is_inserted_and_its_fate_kept() {
    for fate in [
        crate::runner::ValueFate::Inserted,
        crate::runner::ValueFate::NoMatch,
        crate::runner::ValueFate::AmbiguousIgnored,
    ] {
        let absorbed = absorb(Ok(Some(Box::new(7_u32))), |value| {
            // The closure is the only place the value is visible; the driver
            // would pass `|v| ctx.insert_value(v).into()`.
            assert!(value.downcast_ref::<u32>() == Some(&7));
            fate
        });

        let Absorbed { fate: recorded, error } = absorbed;
        assert_eq!(recorded, Some(fate));
        assert!(error.is_none(), "a step that ran has no error");
    }
}

/// A step that returned nothing inserts nothing, and records no fate.
///
/// `None` here is distinct from `Some(ValueFate::NoMatch)`: the former is a
/// step with no return value, the latter a value that reached nowhere. Merging
/// them would erase exactly the signal INV-12 exists to preserve.
#[test]
fn a_step_with_no_return_value_records_no_fate() {
    let mut called = false;
    let absorbed = absorb(Ok(None), |_value| {
        called = true;
        crate::runner::ValueFate::Inserted
    });

    let Absorbed { fate, error } = absorbed;
    assert!(fate.is_none());
    assert!(error.is_none());
    assert!(!called, "there was no value to insert");
}

/// A failing step's error is carried through untouched, and no insertion
/// happens.
///
/// The error is moved, not cloned: `absorb` gets ownership, and the driver
/// then moves it into `Terminal::Fail`, so one original reaches both the
/// recorded step and the terminal. A driver that cloned instead would be
/// recording a copy, which is the weaker guarantee.
#[test]
fn a_failed_step_carries_its_error_and_inserts_nothing() {
    let error = not_found();
    let expected = error.clone();
    let mut called = false;
    let absorbed = absorb(Err(error), |_value| {
        called = true;
        crate::runner::ValueFate::Inserted
    });

    let Absorbed { fate, error } = absorbed;
    assert!(fate.is_none(), "a failed step returns no value");
    assert!(!called);
    assert_eq!(error, Some(expected));
}

// --- classify: the enumerated input classes -----------------------------

#[test]
fn a_returned_value_still_continues() {
    // The value was already absorbed, so the decision sees only that the step
    // succeeded. `NoMatch` at insertion must not become a stop: that is INV-12,
    // and the decision cannot even observe the fate.
    assert!(matches!(classify(None), StepDecision::Continue));
}

#[test]
fn a_skip_is_terminal_and_carries_its_reason() {
    let decision = classify(Some(skip(Some("waiting on upstream"))));

    let StepDecision::Skip { message } = decision else {
        panic!("a skip must be terminal, got {decision:?}");
    };
    assert_eq!(message.as_deref(), Some("waiting on upstream"));
}

#[test]
fn a_skip_without_a_reason_is_still_terminal() {
    let decision = classify(Some(skip(None)));

    let StepDecision::Skip { message } = decision else {
        panic!("a bare skip must be terminal, got {decision:?}");
    };
    assert!(message.is_none());
}

/// Every non-skip `Err` class stops the run, carrying the error untouched.
///
/// The three classes are enumerated rather than sampled so that a fourth
/// added to `ExecutionError` shows up as a compile-time gap in this list when
/// `ExecutionError` stops being `#[non_exhaustive]` — and, until then, as a
/// test that fails to classify it.
#[test]
fn every_non_skip_error_is_a_failure_carrying_the_error_verbatim() {
    let cases = [not_found(), missing_fixtures(), handler_failed()];

    for error in cases {
        let expected = error.clone();
        let decision = classify(Some(error));
        let StepDecision::Fail(carried) = decision else {
            panic!("a failure must be terminal, got {decision:?}");
        };
        // Never a label and never a re-wrap: the same error, out.
        assert_eq!(carried, expected);
    }
}

/// A skip is classified as a skip and not as a failure, even though it
/// arrives as an `Err`.
///
/// This is the discrimination the whole variant list exists for: if it were
/// wrong every skipped scenario would report as failed.
#[test]
fn a_skip_is_not_classified_as_a_failure() {
    assert!(!matches!(
        classify(Some(skip(Some("Nope without a bound key")))),
        StepDecision::Fail(_),
    ));
}

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
        &PERMITTED,
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
    let outcome = assemble(vec![skipped(0)], Some(skip_terminal(None)), &REFUSED);

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
    let outcome = assemble(vec![skipped(0)], Some(skip_terminal(None)), &PERMISSIVE);

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
    let outcome = assemble(vec![skipped(0)], Some(skip_terminal(None)), &PERMITTED);

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
        &PERMITTED,
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
    let outcome = assemble(vec![passed(0), passed(1)], None, &PERMITTED);

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
        &PERMITTED,
    );

    assert_eq!(outcome.steps().len(), 3);
    assert_eq!(outcome.steps().first().map(StepOutcome::status), Some(StepStatus::Skipped));
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
    let outcome = assemble(Vec::new(), None, &PERMITTED);

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
        StepKeyword::Given,
        "a calculator",
        Some(&location()),
        Some(fate),
    );
    let outcome = assemble(vec![step], None, &PERMITTED);

    assert_eq!(outcome.steps().first().and_then(StepOutcome::value_insertion), Some(fate));
}
