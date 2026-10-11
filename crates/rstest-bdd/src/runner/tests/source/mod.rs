//! INV-7's data half: every source an outcome reports is the plan's own.
//!
//! INV-7 has three clauses, and this file discharges the first two: every
//! `StepOutcome::source()` equals the `SourceLocation` the plan supplied, for
//! all four statuses, and `terminal_source()` equals the terminal invocation's
//! source. The third — "no source is read back out of `ExecutionError`" — is
//! discharged here too, by building outcomes whose errors carry a *decoy* path.
//! It holds for the `Display` projection as well as for the accessors:
//! `ScenarioOutcome`'s `Display` re-renders the failure through
//! `ExecutionError::format_with_loader_at` with `terminal_source()`, so the
//! rendering carries the plan's path and agrees with the accessor, and
//! `rendering` asserts that agreement. The one caveat: when
//! `terminal_source()` is `None` — an index past the end of `steps`, or a
//! record with no location — there is no plan-side path to substitute and the
//! rendering falls back to the error's own, which is a fallback rather than a
//! divergence. So the claim this file proves is about the accessors, which is
//! what a frontend walking `steps()` consumes, and about the rendering that
//! now agrees with them.
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

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    sync::Arc,
};

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
        SkipPolicyRecord,
        SkipRecord,
        SourceLocation,
        SourcePath,
        StepOutcome,
        StepStatus,
        ValueFate,
        test_invocation,
    },
};

/// The `Hash` value of a `SourcePath`, for comparing two paths' hashing.
///
/// `SourcePath` implements `Hash` but no caller in the crate hashes one, so
/// there is no production route to assert through. Hashing into a
/// `DefaultHasher` is the shortest way to observe the trait's contribution, and
/// it is deliberately compared between two *equal* values rather than against a
/// fixed number: the contract is that `Eq` and `Hash` agree, not that any
/// particular seed is used.
fn hash_of(path: &SourcePath) -> u64 {
    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);
    hasher.finish()
}

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
        &test_invocation(StepKeyword::Given, "an undefined step", Some(&source)),
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
        &test_invocation(StepKeyword::Given, "a step", Some(&source)),
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

/// The two storage variants of one path are one path (INV-7).
///
/// A parser that reads a document at run time hands back an `Arc<str>`, while
/// the macro path hands back `&'static str`, and both are describing the same
/// source. A derived `PartialEq` compares the discriminant first, so it would
/// call these two unequal and hash them differently — which would make
/// `SourceLocation`'s, `StepOutcome`'s, and `ScenarioOutcome`'s derived
/// equality report two records of one document as different records.
///
/// The `Hash` assertion is not decorative: `Eq` and `Hash` agree here only
/// because both were written over `as_str()`. An `Eq` written by hand beside a
/// *derived* `Hash` would compile, pass an equality-only test, and then lose
/// keys in a map — the inconsistency a comparison-only test cannot see. The
/// hashing control below is what makes this test able to fail for that defect.
#[test]
fn one_path_has_one_identity_whatever_its_storage() {
    let static_path = SourcePath::Static(SPEC_PATH);
    let shared: SourcePath = SourcePath::from(Arc::<str>::from(SPEC_PATH));
    let different: SourcePath = SourcePath::from(Arc::<str>::from(PROSE_PATH));

    assert_eq!(
        static_path, shared,
        "the storage differs, the path does not, so the two must compare equal",
    );
    assert_ne!(
        static_path, different,
        "the control: equality must still separate two genuinely different paths",
    );
    assert_eq!(
        hash_of(&static_path),
        hash_of(&shared),
        "equal values must hash alike, or a map keyed by a path cannot find its own key",
    );
    assert_eq!(
        static_path.as_str(),
        shared.as_str(),
        "and the shared basis is the path text both expose",
    );
}

/// The `SourcePath` equality spreads to the locations that embed it.
///
/// `SourcePath`'s own test above would not catch a `SourceLocation` that
/// hand-wrote a variant-sensitive comparison of its own, and a location is what
/// the outcome's accessors actually return.
#[test]
fn a_static_and_a_shared_location_for_one_path_agree() {
    let static_location = SourceLocation::new_static(SPEC_PATH, 7, Some(3));
    let shared_location =
        SourceLocation::new(SourcePath::from(Arc::<str>::from(SPEC_PATH)), 7, Some(3));
    let other_line = SourceLocation::new(SourcePath::from(Arc::<str>::from(SPEC_PATH)), 8, Some(3));

    assert_eq!(
        static_location, shared_location,
        "one document at one position is one location, however the path is stored",
    );
    assert_ne!(
        static_location, other_line,
        "the control: the line is still part of the location's identity",
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
            SkipRecord {
                message: None,
                source: Some(SourceLocation::new(
                    SourcePath::from(Arc::<str>::from(SPEC_PATH)),
                    7,
                    Some(3),
                )),
            },
            SkipPolicyRecord {
                allow_skipped: true,
                forced_failure: false,
            },
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
            &test_invocation(
                StepKeyword::Given,
                "an undefined step",
                Some(&at(PROSE_PATH, line(2))),
            ),
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
