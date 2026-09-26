//! Handwritten witnesses, one per kind and one per classification.
//!
//! The generator reaches every kind, but a property failure reports the
//! *minimal* shrunk case — the simplest witness rather than the most legible.
//! These rows name each kind concretely so a reader can tell what a
//! classification means without reading a shrink report, and so a kind that
//! stopped being reachable shows up as a wrong status rather than only as a
//! non-vacuity failure an iteration later.
//!
//! Everything here is a control on the *vocabulary* rather than on a generated
//! case, so each one is a named plan run once. The negative controls in
//! `controls.rs` are the complementary half: they corrupt a run and require a
//! predicate to object, where these check a clean run against its declared
//! meaning.

use rstest_bdd::runner::{FailureKind, ScenarioStatus, StepStatus, ValueFate};

use super::{
    collect_witnesses,
    plan,
    sequence::{Arrangement, Kind, Run, run_case},
};

/// One kind, the label its assertions carry, and the fate its invocation must
/// be recorded with.
///
/// The label travels beside the kind rather than being derived from it so that
/// a failing assertion can name the row it came from; assertions here are
/// written per row, so a label read off [`Kind`] would make two rows
/// indistinguishable in the output.
///
/// Duplicate and missing rows are caught by [`every_kind_has_a_label`], by its
/// kind-coverage loop and its length check — not by the label, which no lookup
/// keys on.
///
/// The fate is `Option<StepStatus>` rather than a bare `StepStatus` so the
/// table can state that a *missing* record is a failure of this test: every
/// kind here is terminal, so every one of them leaves a record, and a row whose
/// fate were `None` would be asserting that the driver recorded nothing at the
/// invocation that ended the run. The two kinds that reach a handler and the
/// one that fails before it are distinguished by
/// [`failure_kind`](Kind::failure_kind), not by the presence of a record.
#[derive(Clone, Copy)]
struct Witness {
    /// The name the failure messages prefix themselves with.
    label: &'static str,
    /// Which registered step the invocation names.
    kind: Kind,
    /// What that invocation must be recorded as.
    fate: Option<StepStatus>,
}

/// Every terminal kind, reached by a handwritten plan.
///
/// Each row's comment is the mechanism: *why* this kind ends the run and why it
/// leaves the record it does. The table is shared by [`every_kind_has_a_label`]
/// and [`each_witness_terminates_the_run`], so a kind present in one and absent
/// from the other is not a hole either could see.
const WITNESSES: [Witness; 9] = [
    // Nothing fails, so the run reaches every invocation and records each one.
    Witness {
        label: "Pass",
        kind: Kind::Pass,
        fate: Some(StepStatus::Passed),
    },
    // The value is returned and recorded; recording is not a failure.
    Witness {
        label: "ReturnValue",
        kind: Kind::ReturnValue,
        fate: Some(StepStatus::Passed),
    },
    // Its value matches no fixture, which the driver records and moves past.
    Witness {
        label: "ReturnUnmatchedValue",
        kind: Kind::ReturnUnmatchedValue,
        fate: Some(StepStatus::Passed),
    },
    // Observing resolves the name and records the reading; the run goes on.
    Witness {
        label: "Observe",
        kind: Kind::Observe,
        fate: Some(StepStatus::Passed),
    },
    // A skip request ends the run here rather than failing it.
    Witness {
        label: "Skip",
        kind: Kind::Skip,
        fate: Some(StepStatus::Skipped),
    },
    // A handler error is an assertion-class failure here.
    Witness {
        label: "HandlerError",
        kind: Kind::HandlerError,
        fate: Some(StepStatus::Failed),
    },
    // Resolves to nothing, so it fails here without a handler ever running.
    Witness {
        label: "UnregisteredStep",
        kind: Kind::UnregisteredStep,
        fate: Some(StepStatus::Failed),
    },
    // Validation rejects it before its handler, and the rejection is recorded.
    Witness {
        label: "MissingFixture",
        kind: Kind::MissingFixture,
        fate: Some(StepStatus::Failed),
    },
    // The handler unwinds; the driver's own boundary returns a failure.
    Witness {
        label: "Panic",
        kind: Kind::Panic,
        fate: Some(StepStatus::Failed),
    },
];

/// How many of the control plan's three invocations reach a handler.
///
/// Decided by two questions: does the run stop at index 1, and does index 1
/// reach a handler? A run that stops there never runs the trailing pass, and a
/// kind that ends the run before its handler contributes nothing to the log
/// even though it is the invocation that ended it.
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

/// Every label a witness carries, and the label table's other properties.
///
/// The table must name every kind, and no kind twice: a row deleted from
/// [`Kind::ALL`] is caught by the first assertion, and a witness whose kind
/// stopped being reachable would otherwise be covered up by a second row for a
/// kind that is.
#[test]
fn every_kind_has_a_label() {
    let labels: Vec<&str> = WITNESSES.iter().map(|witness| witness.label).collect();
    assert_eq!(
        labels.len(),
        Kind::ALL.len(),
        "every kind in {KIND_ALL:?} needs a row, or it is only ever witnessed by the generator",
        KIND_ALL = Kind::ALL
    );

    for kind in Kind::ALL {
        assert!(
            WITNESSES.iter().any(|witness| witness.kind == kind),
            "kind {kind:?} has no row naming it",
        );
    }
}

/// Each witnessed kind stops the run at its own invocation, or does not.
///
/// "Reached" is read off the execution log the handlers append to rather than
/// off `outcome.steps()`, because `steps()` records what the driver *says* it
/// did — a driver that ran an invocation and then recorded it as `Bypassed`
/// satisfies every assertion drawn from `steps()` alone.
#[test]
fn each_witness_terminates_the_run() {
    for witness in &WITNESSES {
        let Witness { label, kind, fate } = *witness;
        let (steps, run) = Run::of(kind);
        let describe = run.describe(&steps);

        // The record the invocation itself left behind. Every kind here is
        // terminal, so every one leaves one; `fate: None` would be asserting
        // that the driver recorded nothing at the invocation it stopped on.
        pretty_assertions::assert_eq!(
            run.fate_at(1),
            fate,
            "({label}) the invocation's own record; {describe}",
        );

        // The classification at that invocation, which is a property of the
        // kind rather than of the fate: a failed and a skipped kind share no
        // classification, and a passing one has none.
        pretty_assertions::assert_eq!(
            run.failure_kind_at(1),
            kind.failure_kind(),
            "({label}) the terminal record's classification; {describe}",
        );

        // INV-1's evidence is the log the handlers write. The count is exact
        // rather than a lower bound, which is what makes it evidence: two of
        // the three invocations are the leading and trailing `Pass`, and those
        // log whenever they run — so the difference the count notices is an
        // invocation that reached no handler at all.
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

        // Where the run stopped. "Ended at 1" and "went no further than 1" are
        // different questions and this control needs both: a driver that ran
        // the trailing `Pass` and merely *recorded* it as `Bypassed` answers
        // the first alone. `terminal_exceeded` is a predicate rather than an
        // assertion so `controls.rs` can hand it an overrun and require it to
        // object; the execution-log check asks the same of the handlers.
        assert!(
            !run.terminal_exceeded(),
            "({label}) a terminal of any kind must stop the run; {describe}",
        );
        if kind.terminal_status().is_some() {
            assert!(
                !run.executed.contains(&2),
                "({label}) the invocation after the terminal must not have reached a handler; \
                 {describe}",
            );
            assert_eq!(
                run.terminal_index(),
                Some(1),
                "({label}) the run must stop at the terminal invocation; {describe}",
            );
        }
    }
}

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
