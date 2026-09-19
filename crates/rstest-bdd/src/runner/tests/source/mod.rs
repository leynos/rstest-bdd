//! INV-7's data half: every source an outcome reports is the plan's own.
//!
//! INV-7 has three clauses, and this file discharges the first two: every
//! `StepOutcome::source()` equals the `SourceLocation` the plan supplied, for
//! all four statuses, and `terminal_source()` equals the terminal invocation's
//! source. The third — "no source is read back out of `ExecutionError`" — is
//! discharged here too, by building outcomes whose errors carry a *decoy* path,
//! and it has a caveat worth knowing before reading the results: the
//! *rendering* does read the error's copy, and `rendering` records that. So the
//! claim this file proves is about the accessors, which is what a frontend
//! walking `steps()` consumes, and not about the `Display` projection.
//!
//! # Why the sources are not `.feature` files
//!
//! `spec/cases.toml` and `notes/example.md` are identifiers no Gherkin code
//! path can produce. A test written against `features/x.feature` would pass
//! against an implementation that reconstructed the path from a convention
//! rather than carrying the plan's, and the whole point of the type is that a
//! frontend keeps its own source identity all the way into the outcome.

mod rendering;
mod support;

use std::sync::Arc;

use rstest::rstest;

use self::support::{DECOY_PATH, PROSE_PATH, SPEC_PATH, at, decoy_error, every_status, line};
use crate::{
    StepKeyword,
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

/// Every status's record keeps the location the plan supplied (INV-7).
#[rstest]
#[case::passed(StepStatus::Passed, 0, StepKeyword::Given)]
#[case::skipped(StepStatus::Skipped, 1, StepKeyword::When)]
#[case::failed(StepStatus::Failed, 2, StepKeyword::Then)]
#[case::bypassed(StepStatus::Bypassed, 3, StepKeyword::Then)]
fn every_status_records_the_supplied_location(
    #[case] status: StepStatus,
    #[case] index: usize,
    #[case] keyword: StepKeyword,
) {
    let Some((_, record)) = every_status()
        .into_iter()
        .find(|(candidate, _)| *candidate == status)
    else {
        panic!("every_status must cover {status:?}");
    };

    assert_eq!(record.status(), status, "the row is the status it names");
    assert_eq!(record.index(), index, "the record keeps its index");
    assert_eq!(record.keyword(), keyword, "the record keeps its keyword");
    assert_eq!(
        record.source().map(SourceLocation::path),
        Some(PROSE_PATH),
        "the source comes from the plan, not from the error's decoy {DECOY_PATH}",
    );
    assert_eq!(
        record.source().map(SourceLocation::line),
        Some(line(1)),
        "the per-step line is the plan's, not the scenario's and not zero",
    );
    assert_ne!(
        record.source().map(SourceLocation::path),
        Some(DECOY_PATH),
        "the error's feature_path must not be the source; reading it back is the shortcut INV-7 \
         forbids",
    );
}

/// A failed record's source is the plan's, and the error is still reachable.
///
/// The two claims are asserted together because the temptation INV-7 names is
/// to *replace* the plan's location with the error's, which would keep the
/// failure reachable while losing the fidelity. Asserting only that the error
/// survives would not catch that.
#[test]
fn a_failed_record_keeps_the_plans_path_and_not_the_errors() {
    let source = at(SPEC_PATH, 7);
    let record = StepOutcome::failed(
        0,
        StepKeyword::Given,
        "an undefined step",
        Some(&source),
        decoy_error(0),
    );

    assert_eq!(
        record.error().map(FailureKind::of),
        Some(FailureKind::Undefined),
        "the error itself is retained, so the record explains the failure",
    );
    assert_eq!(
        record.source().map(SourceLocation::path),
        Some(SPEC_PATH),
        "the path is the plan's, even though the error carries one of its own",
    );
    assert_eq!(
        record.source().map(SourceLocation::line),
        Some(7),
        "and the line is the plan's too",
    );
}

/// A non-`'static` source survives into the outcome (INV-7).
///
/// The macro path supplies `&'static str`, so every other test in this file
/// would pass against a `SourcePath` that could only hold a literal. This one
/// supplies an `Arc<str>` built at runtime, which is what a frontend parsing a
/// document into an owned buffer must do, and asserts that the path and the
/// column both round-trip.
///
/// The column is the load-bearing assertion here. A `.feature`-shaped parser
/// has a line and little else; a parser for a structured or prose document is
/// far more likely to know where in the line a step began. Dropping the column
/// when the source crosses into the outcome would lose exactly the position
/// the non-`.feature` frontends this type exists for are best placed to supply.
#[test]
fn a_runtime_built_source_round_trips_through_the_outcome() {
    let owned = SourcePath::from(Arc::<str>::from(SPEC_PATH));
    let source = SourceLocation::new(owned, 42, Some(9));
    let record = StepOutcome::passed(
        0,
        StepKeyword::Given,
        "a step",
        Some(&source),
        Some(ValueFate::NoMatch),
    );

    // A `let ... else` rather than `unwrap_or_else(|| panic!(..))`, which
    // Whitaker's `no_unwrap_or_else_panic` forbids: the same message, but said
    // as a failure to resolve rather than as a default.
    let Some(recorded) = record.source().cloned() else {
        panic!("the record must carry a source; it was built with one");
    };
    assert_eq!(recorded.path(), SPEC_PATH);
    assert_eq!(recorded.line(), 42);
    assert_eq!(
        recorded.column(),
        Some(9),
        "a column the frontend supplied must survive into the outcome",
    );
    assert_eq!(
        record.value_insertion(),
        Some(ValueFate::NoMatch),
        "the control: the record is otherwise ordinary, so the assertions above are about the \
         source rather than about a record that lost everything",
    );
}

/// `terminal_source` reports the location the terminal event happened at.
///
/// The failure half is asserted against an outcome whose step list and failure
/// agree on the index, because that agreement is what lets the accessor look
/// the source up from the step rather than from the error.
#[test]
fn the_terminal_source_is_the_terminating_invocations_location() {
    let skipped = ScenarioOutcome::new(
        ScenarioStatus::Skipped,
        Vec::new(),
        Some(ScenarioSkip::new(
            0,
            None,
            Some(SourceLocation::new(
                SourcePath::from(Arc::<str>::from(SPEC_PATH)),
                7,
                Some(3),
            )),
            true,
            false,
        )),
        None,
    );
    let source = skipped.terminal_source();
    assert_eq!(source.map(SourceLocation::path), Some(SPEC_PATH));
    assert_eq!(source.map(SourceLocation::line), Some(7));
    assert_eq!(source.and_then(SourceLocation::column), Some(3));

    let failed = ScenarioOutcome::new(
        ScenarioStatus::Failed,
        vec![StepOutcome::failed(
            0,
            StepKeyword::Given,
            "an undefined step",
            Some(&at(PROSE_PATH, line(2))),
            decoy_error(0),
        )],
        None,
        Some(ScenarioFailure::Step {
            index: 0,
            error: decoy_error(0),
        }),
    );
    assert_eq!(
        failed.terminal_source().map(SourceLocation::line),
        Some(line(2)),
        "the failing step's own line, not the error's decoy path and not zero",
    );
    assert_eq!(
        failed.terminal_source().map(SourceLocation::path),
        Some(PROSE_PATH),
    );

    let passed = ScenarioOutcome::new(ScenarioStatus::Passed, Vec::new(), None, None);
    assert!(
        passed.terminal_source().is_none(),
        "a run that terminated by exhausting its steps has no terminal event to locate",
    );
}

/// The terminal source for a failure site that names no step.
///
/// An empty plan fails with no invocation to point at, so an accessor that
/// indexed the step list unconditionally would panic here. The assertion is
/// that it returns `None` rather than that it does not panic, because a `None`
/// is what a frontend renders as "no location".
#[test]
fn a_failure_with_no_step_has_no_terminal_source() {
    let empty = ScenarioOutcome::new(
        ScenarioStatus::Failed,
        Vec::new(),
        None,
        Some(ScenarioFailure::EmptyPlan),
    );
    assert!(empty.terminal_source().is_none());

    let phantom = ScenarioOutcome::new(
        ScenarioStatus::Failed,
        Vec::new(),
        None,
        Some(ScenarioFailure::Step {
            index: 3,
            error: decoy_error(3),
        }),
    );
    assert!(
        phantom.terminal_source().is_none(),
        "an index past the end of the step list is not a location; the accessor must not index \
         into the list with it",
    );
    assert_eq!(
        phantom.failure().map(ScenarioFailure::site),
        Some(FailureSite::Step(3)),
        "the control: the failure is still reported with its site, so the `None` above is about \
         the source lookup rather than about a lost failure",
    );
}
