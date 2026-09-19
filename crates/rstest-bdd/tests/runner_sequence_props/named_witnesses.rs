//! Handwritten witnesses, one per kind and one per classification.
//!
//! The generator reaches every kind — `Witnesses::assert_complete` is what
//! holds it to that — but a property failure reports the *minimal* shrunk case,
//! which is by construction the simplest witness rather than the most legible.
//! These rows name each kind concretely so a reader can tell what a
//! classification means without reading a shrink report, and so a kind that
//! stopped being reachable shows up as a wrong status here rather than only as
//! a non-vacuity failure an iteration later.
//!
//! Everything here is a control on the *vocabulary* rather than on a generated
//! case, so each one is a named plan run once. The negative controls in
//! `controls.rs` are the complementary half: they corrupt a run and require a
//! predicate to object, where these check a clean run against its declared
//! meaning.

use rstest_bdd::runner::{FailureKind, ScenarioStatus, StepOutcome, ValueFate};

use super::{
    collect_witnesses,
    plan,
    sequence::{Arrangement, Kind, run_case},
};

/// INV-12's three fates, each pinned to the arrangement that produces it.
///
/// The property in `invariants.rs` asserts all three are *reached*, which a
/// generator could manage without any of them following from the fixture
/// arrangement. These rows name the mechanism: one matching fixture inserts,
/// none leaves the value unmatched, two make it ambiguous.
#[test]
fn the_three_fates_follow_from_the_fixture_arrangement() {
    let rows = [
        (
            "one fixture in the value's type",
            Arrangement::OneProbe,
            ValueFate::Inserted,
        ),
        (
            "no fixture in the value's type",
            Arrangement::NoProbe,
            ValueFate::NoMatch,
        ),
        (
            "two fixtures in the value's type",
            Arrangement::TwoProbes,
            ValueFate::AmbiguousIgnored,
        ),
    ];

    for (label, arrangement, expected) in rows {
        let steps = plan(&[Kind::ReturnValue]);
        let run = run_case(&steps, arrangement);
        assert_eq!(
            run.outcome.status(),
            ScenarioStatus::Passed,
            "({label}) a recorded or dropped value is not itself a failure; {}",
            run.describe(&steps),
        );
        assert_eq!(
            run.fates(&steps),
            vec![(0, Some(expected))],
            "({label}) the fate must be {expected:?}",
        );
    }
}

/// The generator reaches each terminal kind, and reaches enough of each.
///
/// [`Witnesses::assert_complete`](super::sequence::Witnesses::assert_complete)
/// proves each class *occurred*; this proves each occurred often enough to be a
/// property of the strategy rather than a lucky draw. A uniform draw over
/// [`Kind::ALL`] reaches `Skip` about a ninth of the time per invocation, so
/// "it happened at least once in 256 cases" is weak evidence for the classes
/// that need a *position* as well: a terminal kind only stops a run when
/// something follows it, and `NoMatch` needs a returning step whose kind was
/// drawn at all.
///
/// Flooring the counts turns that into evidence, but the floor has to sit below
/// the strategy's own *minimum* rather than below its average — and the two are
/// not close. Measured over 200 independent runs of this test (51,200 generated
/// cases), the per-class counts ranged:
///
/// ```plaintext
/// passed    min= 36 p5= 41 mean= 51.1
/// skipped   min= 28 p5= 37 mean= 46.3
/// inserted  min= 37 p5= 45 mean= 58.1
/// nomatch   min= 55 p5= 63 mean= 77.4
/// ```
///
/// `skipped` is the sparse class, and its minimum lands *below* the mean by
/// more than a third. The floor is therefore 20 rather than 30: 30 was observed
/// to fail once in those 200 runs, which would be an intermittent red on a
/// green tree — the worst kind of failure, because the next step is to re-run
/// rather than to investigate. A floor of 20 sits about 1.4× below the
/// observed minimum and about 2.3× below the mean, so it still fails on any
/// regression that materially changes a rate (a halved `skipped` rate lands
/// near 23 and would be caught about half the time, and a class that stopped
/// being reached fails immediately) while retiring the false red.
///
/// The `mean` is what a reader is likely to reach for instead, and it is the
/// wrong number: an earlier draft of this comment cited "about 46" as though it
/// were the floor's headroom, when it is the centre of a distribution whose
/// lower tail crosses 30.
#[test]
fn the_generator_reaches_each_class_often_enough() {
    let tally = collect_witnesses().tally();

    assert!(
        tally.cases >= 200,
        "the run classified {} cases, so a floor of 20 would say nothing",
        tally.cases,
    );
    for expected in [ScenarioStatus::Passed, ScenarioStatus::Skipped] {
        let count = tally.status(expected);
        assert!(
            count >= 20,
            "only {count} of {} runs ended {expected:?}; a class this rare is one the generator \
             has stopped reaching rather than one it sometimes misses",
            tally.cases,
        );
    }
    for expected in [ValueFate::Inserted, ValueFate::NoMatch] {
        let count = tally.fate(expected);
        assert!(
            count >= 20,
            "only {count} value-returning invocations recorded {expected:?} across {} cases",
            tally.cases,
        );
    }
}

/// Every terminal kind, reached by a handwritten plan.
///
/// The label travels with the kind rather than being derived from it, so the
/// lookup cannot be fooled by the constant it is looking for: a pair whose
/// kinds were both `Pass` fails on the duplicated label instead of passing on
/// the constant.
#[test]
fn each_terminal_kind_is_reached_by_its_own_witness() {
    let pairs = [
        ("Pass", Kind::Pass),
        ("ReturnValue", Kind::ReturnValue),
        ("ReturnUnmatchedValue", Kind::ReturnUnmatchedValue),
        ("Observe", Kind::Observe),
        ("Skip", Kind::Skip),
        ("HandlerError", Kind::HandlerError),
        ("UnregisteredStep", Kind::UnregisteredStep),
        ("MissingFixture", Kind::MissingFixture),
        ("Panic", Kind::Panic),
    ];
    let labels: Vec<&str> = pairs.iter().map(|(label, _)| *label).collect();
    assert_eq!(
        labels.len(),
        Kind::ALL.len(),
        "every kind in {KIND_ALL:?} needs a row, or it is only ever witnessed by the generator",
        KIND_ALL = Kind::ALL
    );
    for kind in Kind::ALL {
        assert!(
            pairs.iter().any(|(_, row)| *row == kind),
            "kind {kind:?} has no row naming it",
        );
    }

    for (label, kind) in pairs {
        let steps = plan(&[Kind::Pass, kind, Kind::Pass]);
        let run = run_case(&steps, Arrangement::OneProbe);
        let describe = run.describe(&steps);

        match kind.terminal_status() {
            None => {
                assert_eq!(
                    run.outcome.status(),
                    ScenarioStatus::Passed,
                    "({label}) a non-terminal kind must let the run continue; {describe}",
                );
                assert_eq!(
                    run.executed,
                    vec![0, 1, 2],
                    "({label}) every invocation must run; {describe}",
                );
            }
            Some(expected) => {
                assert_eq!(
                    run.outcome.status(),
                    expected,
                    "({label}) must end the run as {expected:?}; {describe}",
                );
                assert_eq!(
                    run.terminal_index(),
                    Some(1),
                    "({label}) the run must stop at the terminal invocation; {describe}",
                );
                assert_eq!(
                    run.outcome
                        .steps()
                        .get(1)
                        .and_then(StepOutcome::failure_kind),
                    kind.failure_kind(),
                    "({label}) the terminal record's classification; {describe}",
                );
            }
        }
        assert!(
            !run.terminal_exceeded(),
            "({label}) a terminal of any kind must stop the run; {describe}",
        );

        // INV-1's evidence is the log the handlers write. A kind whose handler
        // never records leaves an invocation with no trace at all, so a driver
        // that ran it past the terminal would be invisible to the predicate.
        //
        // The count is exact rather than a lower bound, which is what makes it
        // evidence: two of the three invocations are the leading and trailing
        // `Pass`, and those log whenever they run. So a kind that reaches no
        // handler still leaves the invocations *before* it logged, and its
        // absence from the log is the difference the count notices.
        assert_eq!(
            run.executed.len(),
            expected_executed(kind),
            "({label}) the log must hold exactly the invocations that reached a handler ({} of \
             them), or INV-1's evidence has a hole in it; {describe}",
            expected_executed(kind),
        );
        if !kind.logs_its_position() {
            assert!(
                !run.executed.contains(&1),
                "({label}) this kind never reaches a handler, so the terminal invocation must not \
                 be in the log; {describe}",
            );
        }
    }
}

/// How many of the control plan's three invocations reach a handler.
///
/// The control is always `Pass, kind, Pass`, so the count is decided by two
/// questions: does the run stop at index 1, and does index 1 itself reach a
/// handler? A run that stops there never runs the trailing pass; and a kind
/// that ends the run *before* its handler is called contributes nothing to the
/// log even though it is the invocation that ended the run. Both such kinds are
/// resolved before the call — an unregistered pattern finds no step at all, and
/// a missing fixture is rejected by validation upstream of the handler.
fn expected_executed(kind: Kind) -> usize {
    match (kind.terminal_status(), kind.logs_its_position()) {
        // Nothing stops the run, so every invocation reaches its handler.
        (None, _) => 3,
        // The run stops at index 1, whose own handler ran and logged.
        (Some(_), true) => 2,
        // The run stops at index 1 before its handler, so only index 0 logged.
        (Some(_), false) => 1,
    }
}

/// Every classification the plan's INV-1 list names has a kind that produces it.
///
/// [`Witnesses::assert_complete`](super::sequence::Witnesses::assert_complete)
/// says the generator reached each class; this says each class is reachable
/// from a kind at all, and that the kind's own declared classification is the
/// one the run reports. A kind deleted from [`Kind::ALL`] fails the first; a
/// kind whose `failure_kind` was changed to `None` would quietly stop
/// contributing to it, and fails here.
#[test]
fn every_classification_the_plan_names_has_a_kind() {
    for expected in [
        FailureKind::Assertion,
        FailureKind::Undefined,
        FailureKind::MissingFixture,
        FailureKind::Panic,
    ] {
        let mut rows = Kind::ALL
            .into_iter()
            .filter(|kind| kind.failure_kind() == Some(expected));
        let source = rows.next().unwrap_or_else(|| {
            panic!(
                "no kind in {KIND_ALL:?} classifies as {expected:?}, so the plan's INV-1 \
                 non-vacuity list cannot be satisfied by any generated case",
                KIND_ALL = Kind::ALL
            )
        });
        assert!(
            rows.next().is_none(),
            "{expected:?} is claimed by more than one kind, so this control does not pin it",
        );

        let run = run_case(&plan(&[source]), Arrangement::OneProbe);
        assert_eq!(
            run.failure_kinds(),
            vec![expected],
            "the one-invocation witness for {expected:?} must classify as it",
        );
    }
}
