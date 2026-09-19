//! Tests for the canonical fold and the outcome's accessors.
//!
//! Everything here builds outcomes synthetically rather than running a
//! scenario. That is deliberate: the fold is a pure function of the outcome's
//! fields, so its cases are enumerable without a registry, and a synthetic
//! outcome is also what the integration suite's negative controls need.

use std::sync::Arc;

use crate::{
    StepError,
    StepKeyword,
    execution::ExecutionError,
    runner::{
        FailureKind,
        FailureSite,
        ScenarioFailure,
        ScenarioOutcome,
        ScenarioSkip,
        ScenarioStatus,
        SourceLocation,
        SourcePath,
        StepOutcome,
        StepStatus,
        ValueFate,
    },
};

/// The location most tests in this file give to a step.
fn location(line: u32) -> SourceLocation { SourceLocation::new_static("notes/demo.md", line, None) }

/// An `ExecutionError::StepNotFound`, the shape most tests here fail with.
fn not_found(index: usize) -> ExecutionError {
    ExecutionError::StepNotFound {
        index,
        keyword: StepKeyword::Given,
        text: "an undefined step".to_owned(),
        feature_path: "notes/demo.md".to_owned(),
        scenario_name: "demo".to_owned(),
    }
}

/// A one-step outcome that failed at `index` with `error`, whose step record at
/// that index carries the same error and the supplied source location.
///
/// Building both from one error keeps the step list and the terminal failure
/// consistent, which is the invariant the runner must maintain; a test that
/// manufactured two different errors could not tell a correct fold from one
/// that re-derived the failure from the wrong place.
fn failing_outcome(index: usize, error: ExecutionError) -> ScenarioOutcome {
    let step = StepOutcome::failed(
        index,
        StepKeyword::Given,
        "an undefined step",
        Some(&location(12)),
        error.clone(),
    );
    ScenarioOutcome::new(
        ScenarioStatus::Failed,
        vec![step],
        None,
        Some(ScenarioFailure::Step { index, error }),
    )
}

/// A one-step passing outcome, used as the non-vacuity control: the cases that
/// must fail are only meaningful if an ordinary run still succeeds.
fn passing_outcome() -> ScenarioOutcome {
    let step = StepOutcome::passed(
        0,
        StepKeyword::Given,
        "a calculator",
        Some(&location(12)),
        None,
    );
    ScenarioOutcome::new(ScenarioStatus::Passed, vec![step], None, None)
}

/// A skipped outcome whose `forced_failure` is the supplied value.
fn skipping_outcome(forced_failure: bool) -> ScenarioOutcome {
    let step = StepOutcome::skipped(
        0,
        StepKeyword::Given,
        "a pending step",
        Some(&location(12)),
        Some("waiting on upstream".to_owned()),
    );
    let skip = ScenarioSkip::new(
        0,
        Some("waiting on upstream".to_owned()),
        Some(location(12)),
        !forced_failure,
        forced_failure,
    );
    ScenarioOutcome::new(ScenarioStatus::Skipped, vec![step], Some(skip), None)
}

#[test]
fn passing_outcome_folds_to_ok() {
    assert_eq!(passing_outcome().into_harness_result(), Ok(()));
}

/// The discriminating case for skip policy: a scenario that *permits* skipping
/// still fails the suite when policy forces it.
#[test]
fn canonical_fold_folds_forced_skip() {
    let outcome = skipping_outcome(true);

    assert_eq!(outcome.status(), ScenarioStatus::Skipped);
    assert!(outcome.skip().is_some_and(ScenarioSkip::forced_failure));
    // `is_passed` deliberately does *not* fold policy, so the two must agree
    // that this is not a clean pass while disagreeing about why.
    assert!(!outcome.is_passed());

    let result = outcome.into_harness_result();
    let Err(failure) = result else {
        panic!("a forced skip must not fold to a clean pass");
    };
    assert_eq!(failure.site(), FailureSite::ForcedSkip(0));
}

/// The mirror case: an allowed skip is not a failure, and folding it yields
/// `Ok(())` even though the status is `Skipped`.
#[test]
fn canonical_fold_accepts_an_unforced_skip() {
    let outcome = skipping_outcome(false);
    assert_eq!(outcome.status(), ScenarioStatus::Skipped);
    assert_eq!(outcome.into_harness_result(), Ok(()));
}

/// The negative control for the empty plan: a plan with no steps must not fold
/// to `Ok(())`, because a malformed document would then pass the suite.
#[test]
fn canonical_fold_rejects_an_empty_plan() {
    let outcome = ScenarioOutcome::new(ScenarioStatus::Passed, Vec::new(), None, None);

    let result = outcome.into_harness_result();
    let Err(failure) = result else {
        panic!("an empty plan must not fold to a clean pass");
    };
    assert_eq!(failure.site(), FailureSite::EmptyPlan);
    assert!(failure.error().is_none());
}

#[test]
fn canonical_fold_returns_the_step_failure() {
    let error = not_found(0);
    let outcome = failing_outcome(0, error.clone());

    let Err(failure) = outcome.into_harness_result() else {
        panic!("a failing step must not fold to a clean pass");
    };
    assert_eq!(failure.site(), FailureSite::Step(0));

    // The payload travels with the site rather than being re-derived from the
    // step list, so the two cannot disagree: the error the step recorded is the
    // error the fold reports, down to the value.
    assert_eq!(failure.error(), Some(&error));
    assert_eq!(FailureKind::of(&error), FailureKind::Undefined);
}

/// A failed step outranks a skip: once a step has failed, a later skip record
/// must not mask it. Built by hand because the runner cannot produce it — which
/// is exactly why the fold has to be checked against it.
#[test]
fn canonical_fold_prefers_the_failure_over_a_skip() {
    let error = not_found(0);
    let skip = ScenarioSkip::new(0, None, None, true, true);
    let step = StepOutcome::failed(
        0,
        StepKeyword::Given,
        "an undefined step",
        None,
        error.clone(),
    );

    let outcome = ScenarioOutcome::new(
        ScenarioStatus::Failed,
        vec![step],
        Some(skip),
        Some(ScenarioFailure::Step { index: 0, error }),
    );

    let Err(failure) = outcome.into_harness_result() else {
        panic!("a failed step must outrank any skip record");
    };
    assert_eq!(failure.site(), FailureSite::Step(0));
}

#[test]
fn statuses_and_payloads_agree() {
    let step = StepOutcome::passed(
        3,
        StepKeyword::Then,
        "the result is 4",
        Some(&location(45)),
        Some(ValueFate::Inserted),
    );
    assert_eq!(step.status(), StepStatus::Passed);
    assert_eq!(step.index(), 3);
    assert_eq!(step.keyword(), StepKeyword::Then);
    assert_eq!(step.text(), "the result is 4");
    assert_eq!(step.source().map(SourceLocation::line), Some(45));
    assert_eq!(step.value_insertion(), Some(ValueFate::Inserted));
    assert!(step.error().is_none());
    assert!(step.skip_message().is_none());
    assert!(step.failure_kind().is_none());

    let skipped = StepOutcome::skipped(
        1,
        StepKeyword::When,
        "a pending step",
        None,
        Some("later".to_owned()),
    );
    assert_eq!(skipped.status(), StepStatus::Skipped);
    assert_eq!(skipped.skip_message(), Some("later"));
    assert!(skipped.value_insertion().is_none());

    let bypassed = StepOutcome::bypassed(2, StepKeyword::Then, "the result is 4", None);
    assert_eq!(bypassed.status(), StepStatus::Bypassed);
    assert!(bypassed.error().is_none());
    assert!(bypassed.skip_message().is_none());
    // `source` is available for every status, including `Bypassed`, even when
    // the plan recorded none.
    assert!(bypassed.source().is_none());
}

/// The classification is a total function, and each variant is reachable.
///
/// A table rather than a loop over generated errors, because the interesting
/// property is exactly that every *distinguished* case maps where a reporter
/// expects; an `Other` catch-all that swallowed a real case would still look
/// total.
#[test]
fn failure_kinds_are_stable() {
    fn handler_failed(error: StepError) -> ExecutionError {
        ExecutionError::HandlerFailed {
            index: 0,
            keyword: StepKeyword::Given,
            text: "x".to_owned(),
            error: Arc::new(error),
            feature_path: "f".to_owned(),
            scenario_name: "s".to_owned(),
        }
    }

    let cases = [
        (not_found(0), FailureKind::Undefined),
        (
            ExecutionError::MissingFixtures(Arc::new(crate::execution::MissingFixturesDetails {
                step_pattern: "x".to_owned(),
                step_location: "f:1".to_owned(),
                required: Vec::new(),
                missing: Vec::new(),
                missing_requirements: Vec::new(),
                available: Vec::new(),
                has_suggestion: false,
                feature_path: "f".to_owned(),
                scenario_name: "s".to_owned(),
            })),
            FailureKind::MissingFixture,
        ),
        (
            handler_failed(StepError::MissingFixture {
                name: "db".to_owned(),
                ty: "Pool".to_owned(),
                step: "Given a database".to_owned(),
            }),
            FailureKind::MissingFixture,
        ),
        (
            handler_failed(StepError::ExecutionError {
                pattern: "x".to_owned(),
                function: "f".to_owned(),
                message: "boom".to_owned(),
            }),
            FailureKind::Assertion,
        ),
        (
            handler_failed(StepError::PanicError {
                pattern: "x".to_owned(),
                function: "f".to_owned(),
                message: "boom".to_owned(),
            }),
            FailureKind::Panic,
        ),
        (
            ExecutionError::Skip {
                message: Some("later".to_owned()),
            },
            FailureKind::Other,
        ),
    ];

    for (error, expected) in cases {
        assert_eq!(FailureKind::of(&error), expected, "for {error:?}");
    }
}

/// `terminal_source` reports the skip's location when the run skipped, and the
/// failing step's location when it failed.
#[test]
fn terminal_source_prefers_the_skip_then_the_failure() {
    let shared: SourcePath = Arc::<str>::from("spec/cases.toml").into();
    let skipped = ScenarioSkip::new(
        0,
        None,
        Some(SourceLocation::new(shared, 7, Some(3))),
        true,
        false,
    );
    let outcome = ScenarioOutcome::new(ScenarioStatus::Skipped, Vec::new(), Some(skipped), None);
    let source = outcome.terminal_source();
    assert_eq!(source.map(SourceLocation::path), Some("spec/cases.toml"));
    assert_eq!(source.map(SourceLocation::line), Some(7));
    assert_eq!(source.and_then(SourceLocation::column), Some(3));

    // A step failure records the index, and the source is looked up from the
    // step list — so the index and the step must agree, as they do here.
    let failed = failing_outcome(0, not_found(0));
    assert_eq!(failed.terminal_source().map(SourceLocation::line), Some(12));

    let empty = ScenarioOutcome::new(ScenarioStatus::Passed, Vec::new(), None, None);
    assert!(empty.terminal_source().is_none());
}
