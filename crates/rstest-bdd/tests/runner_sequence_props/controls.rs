//! The negative controls, and the checks on the generator's own domain.
//!
//! A property that asserts a predicate never fires is worth nothing unless the
//! predicate *can* fire. Each predicate in `sequence::run` is a pure function of
//! a run, so a control here can hand it a run whose log or readings have been
//! corrupted and require it to object. A predicate that had been changed to
//! return `None` unconditionally would pass every property and fail here.
//!
//! The remaining tests check the strategy's own claims: that its plans stay
//! inside the bound the case budget was pinned against, and that the log the
//! properties read is populated by the *handlers* rather than by the driver,
//! which is what makes INV-1's evidence independent of what the driver says.

use proptest::{prelude::*, test_runner::TestRunner};
use rstest_bdd::{
    StepKeyword,
    runner::{FailureKind, ScenarioStatus},
};

use super::{
    plan,
    sequence::{
        Arrangement,
        CASES,
        Kind,
        MAX_STEPS,
        Reading,
        SENTINEL,
        Step,
        Witnesses,
        case,
        crafted,
        executed,
        run_case,
    },
};

/// The negative control for INV-1.
///
/// [`no_invocation_past_the_terminal_index_is_executed`](super::invariants::no_invocation_past_the_terminal_index_is_executed)
/// asserts that a predicate never fires, which is worth nothing unless the
/// predicate *can* fire. `first_exceeding` is a pure function of a run, so this
/// can hand it a run whose log has been corrupted and require it to object. A
/// helper that returned `None` unconditionally would pass the property and fail
/// here.
#[test]
fn the_terminal_predicate_rejects_a_log_that_exceeds_it() {
    let steps = plan(&[Kind::Skip, Kind::Pass]);
    let run = run_case(&steps, Arrangement::OneProbe);

    assert_eq!(
        run.terminal_index(),
        Some(0),
        "the control's plan must stop at the skip: {}",
        run.describe(&steps),
    );
    assert!(
        !run.terminal_exceeded(),
        "and must otherwise be clean: {}",
        run.describe(&steps),
    );

    let mut corrupted = run.clone();
    corrupted.executed.push(1);
    assert_eq!(
        corrupted.first_exceeding(),
        Some(1),
        "a log recording an execution past the terminal index must be rejected; {corrupted:?}",
    );

    let mut beyond = run;
    beyond.executed.push(7);
    assert!(
        beyond.terminal_exceeded(),
        "an index past the plan's own length must be rejected too; {beyond:?}",
    );
}

/// The negative control for INV-3.
///
/// A trace in which an observer reads a *later* producer's value is exactly
/// what the property forbids. Requiring the predicate to reject one is what
/// separates "no case violated it" from "the predicate cannot say so".
#[test]
fn the_visibility_predicate_rejects_an_observer_reading_a_later_producer() {
    let steps = plan(&[Kind::Observe, Kind::ReturnValue]);
    let run = run_case(&steps, Arrangement::OneProbe);

    assert_eq!(
        run.outcome.status(),
        ScenarioStatus::Passed,
        "the control's plan must run cleanly: {}",
        run.describe(&steps),
    );
    assert_eq!(
        run.readings.first().map(|reading| reading.value),
        Some(Some(SENTINEL)),
        "the observer ran before any producer, so it must see the sentinel: {}",
        run.describe(&steps),
    );
    assert!(
        run.visibility_violation().is_none(),
        "and that is not a violation: {}",
        run.describe(&steps),
    );

    let mut corrupted = run;
    if let Some(reading) = corrupted.readings.first_mut() {
        reading.value = Some(1);
    }
    assert!(
        corrupted
            .visibility_violation()
            .is_some_and(|violation| violation.contains("read producer 1")),
        "an observer reading a later producer's value must be rejected; {corrupted:?}",
    );
}

/// The log the properties read is populated by handlers, not by the driver.
///
/// An observer that silently recorded nothing, or a returning step that never
/// logged, would satisfy INV-1 and INV-3 vacuously — so this asserts both logs
/// are populated by a plan that plainly runs, and that the execution log the
/// module holds is the same one a run carries.
#[test]
fn the_logs_the_properties_read_are_populated() {
    let steps = plan(&[Kind::ReturnValue, Kind::Observe]);
    let run = run_case(&steps, Arrangement::OneProbe);

    assert_eq!(
        run.readings,
        vec![Reading {
            observer: 1,
            value: Some(0),
        }],
        "the observer at 1 must read producer 0's value, not the sentinel: {}",
        run.describe(&steps),
    );
    assert_eq!(
        run.executed,
        vec![0, 1],
        "and both handlers must have logged their positions: {}",
        run.describe(&steps),
    );
    assert_eq!(
        executed(),
        run.executed,
        "the module-level log is the same log the run carried",
    );
    assert_eq!(
        run.visibility_violation(),
        None,
        "a producer seen only by a later observer is the invariant holding",
    );
}

/// A keyword a step was not registered under resolves to nothing.
///
/// This is the fact that makes a step's keyword a property of its kind rather
/// than of its position: `And` and `But` exist as [`StepKeyword`] variants, but
/// no attribute macro registers a step under them and `resolve_step` filters on
/// keyword equality. Assigning keywords by position — `index % 3` over
/// `Given`/`When`/`Then` — would therefore send most invocations at a step
/// registered under a different keyword, resolving to nothing and collapsing
/// every kind into `Undefined`. The generator's keyword coupling was changed
/// because of this; the test keeps it from being changed back.
#[test]
fn a_keyword_mismatch_resolves_to_nothing() {
    let steps = vec![
        Step {
            kind: Kind::HandlerError,
            keyword: StepKeyword::And,
            line: 3,
        },
        Step {
            kind: Kind::Pass,
            keyword: StepKeyword::Given,
            line: 4,
        },
    ];
    let run = run_case(&steps, Arrangement::OneProbe);

    assert_eq!(
        run.outcome.status(),
        ScenarioStatus::Failed,
        "an unregistered keyword cannot resolve: {}",
        run.describe(&steps),
    );
    assert_eq!(
        run.failure_kinds(),
        vec![FailureKind::Undefined],
        "and classifies as undefined rather than as the step it named: {}",
        run.describe(&steps),
    );
    assert!(
        run.executed.is_empty(),
        "the named step must not have run: {}",
        run.describe(&steps),
    );
}

/// The crafted catalogue alone witnesses INV-3's negative visibility clause.
///
/// # Why this is separate from the property that asserts it
///
/// `invariants::a_returned_value_is_visible_only_after_its_producer` folds its
/// [`Witnesses`] across both halves of the strategy, and only the biased half
/// draws the catalogue. So the folded assertion can be satisfied *entirely* by
/// the uniform backdrop and still pass — which is exactly what happened: the
/// catalogue had no shape placing an observer before a producer, and the clause
/// rested on the backdrop drawing one, at about one case in fifty. Measured over
/// the pinned budget that was a failing assertion on roughly two runs in five,
/// and it was misread more than once as an intermittent environment fault rather
/// than as a catalogue that had stopped covering the clause it claimed to.
///
/// Driving [`crafted`] alone removes the backdrop from the evidence, so this
/// fails whenever the catalogue loses its witness — which is the claim the
/// catalogue's own documentation makes and the one nothing else checks.
#[test]
fn the_crafted_catalogue_alone_witnesses_the_visibility_clause() {
    let mut witnesses = Witnesses::default();

    for steps in crafted() {
        // `OneProbe` because the witness requires a context capable of showing
        // the observer a value at all; under either other arrangement no insert
        // can succeed, so the observation is true of every driver and the
        // witness would be set on evidence that discriminates nothing. The
        // arrangement is therefore pinned here rather than drawn, and the
        // property that *does* draw it keeps the two independent.
        let run = run_case(&steps, Arrangement::OneProbe);
        assert!(
            run.visibility_violation().is_none(),
            "a crafted plan violates INV-3; {}",
            run.describe(&steps),
        );
        witnesses.record(&steps, Arrangement::OneProbe, &run);
    }

    witnesses.assert_visibility_complete();
}

/// The generator's plan length stays inside the declared domain.
///
/// A strategy that could exceed [`MAX_STEPS`] would make the per-case cost
/// unbounded, which is the whole reason the budget above is worth pinning. The
/// bound is what [`SENTINEL`] and [`CASES`] are calibrated against too: a plan
/// longer than the declared maximum would put a producer's index outside the
/// range the sentinel is chosen to stay clear of.
#[test]
fn the_generator_stays_within_its_declared_bound() {
    let mut runner = TestRunner::new(proptest::test_runner::Config {
        cases: CASES,
        ..proptest::test_runner::Config::default()
    });
    let result = runner.run(&case(), |(steps, _)| {
        prop_assert!(
            steps.len() <= MAX_STEPS,
            "the strategy produced {} steps, above the declared bound {}",
            steps.len(),
            MAX_STEPS
        );
        prop_assert!(
            steps
                .iter()
                .enumerate()
                .all(|(index, step)| step.line == u32::try_from(index).unwrap_or(0) + 1),
            "the strategy produced non-increasing lines: {:?}",
            steps.iter().map(|step| step.line).collect::<Vec<_>>(),
        );
        Ok(())
    });

    assert!(result.is_ok(), "the property failed: {result:?}");
}
