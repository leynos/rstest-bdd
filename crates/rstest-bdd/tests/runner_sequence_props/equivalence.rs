//! INV-5: the synchronous and asynchronous runners produce equal outcomes.
//!
//! `run_scenario` and `run_scenario_async` are separate drivers, and D6 says the
//! only thing that may differ between them is how a step is awaited. This
//! property runs one generated plan through both and compares the whole
//! [`Run`] — the outcome *and* the two logs the rest of the suite treats as
//! independent evidence — so a driver that classified a failure differently,
//! recorded a fate differently, ordered its records differently, or executed
//! operations the other did not is caught whichever of those it drifted in.
//!
//! # Why the comparison is on the whole value
//!
//! A handwritten projection would be the invariant's own weak point. Comparing
//! status alone, or status plus the step count, would leave every other field
//! free to differ, and the fields two drivers are most likely to disagree on —
//! [`ValueFate`](rstest_bdd::runner::ValueFate),
//! [`FailureKind`](rstest_bdd::runner::FailureKind), the recorded source — are
//! exactly the ones a projection would omit. [`Run`]'s `PartialEq` is derived,
//! so what is compared is defined by the types rather than by a list someone
//! has to remember to extend.
//!
//! # Why this is not `pretty_assertions::assert_eq!`
//!
//! The plan names that macro, and this property uses
//! [`prop_assert!`](proptest::prop_assert) with [`Comparison`] in the message
//! instead — deliberately, and it is the same diff. `assert_eq!` *panics*, and
//! a panic inside a proptest case aborts the case rather than reporting a
//! failure, so the counter-example is never shrunk to the plan that actually
//! differs. A property over generated plans whose failures are unshrunk would
//! be much harder to act on. `prop_assert!` reports through the macro's error
//! channel, which keeps shrinking, and `Comparison` renders the identical diff.
//!
//! # Why the two legs do not overlap
//!
//! The steps log their positions to thread-locals (see `steps/mod.rs`), so each
//! leg is run to completion and read back before the next begins. Running them
//! concurrently would interleave the logs and make every comparison a statement
//! about a mixture of both.
//!
//! # Non-vacuity
//!
//! Equality is satisfied by a generator that only ever produced full passes, so
//! the classification asserts that terminal skips, terminal failures, and full
//! passes all occurred. [`the_comparison_rejects_a_mutated_outcome`] is the
//! control that the comparison itself can report a difference; the plan's named
//! control for this invariant is the `cargo-mutants` pass over
//! `runner/engine/drive_async.rs`.
//!
//! # What this does not establish
//!
//! `Async`-only steps have no synchronous counterpart, so they are outside this
//! invariant's domain; INV-15 covers them. The claim "the two loops differ only
//! by `.await`" is a design intent that this property supports for `Both`-mode
//! steps and does not establish in general. The plan records that gap rather
//! than glossing it.

use pretty_assertions::Comparison;
use proptest::prelude::*;

use super::{
    check,
    sequence::{Arrangement, Kind, Run, Step, Witnesses, run_case, run_case_async},
};

/// INV-5: both runners produce equal outcomes for a `Both`-mode plan.
///
/// The domain is the generator's, unchanged: every step the suite registers is
/// `StepExecutionMode::Both` or is not registered at all, and an unresolvable
/// invocation is a failure mode in exactly the same way through both drivers.
///
/// The property asserts *three* things rather than one, and the third is the
/// one that makes the other two safe. "The outcomes are equal" is satisfiable
/// by a generator that only ever produced full passes, and "the logs are equal"
/// by one that produced no plans at all; `assert_complete` runs afterwards and
/// is what stops either from being satisfied that way.
#[test]
fn the_two_runners_produce_equal_outcomes() {
    let mut witnesses = Witnesses::default();

    check(|steps, arrangement| {
        let sync = run_case(&steps, arrangement);
        let asynchronous = run_case_async(&steps, arrangement);
        witnesses.record(&steps, arrangement, &sync);

        prop_assert!(
            sync == asynchronous,
            "the two runners disagreed; {}",
            difference(&steps, arrangement, &sync, &asynchronous),
        );
        Ok(())
    });

    witnesses.assert_complete();
}

/// Describe a disagreement, or say that there is none.
///
/// The comparison is rendered field by field rather than as one whole-value
/// diff, because `Run` holds two logs that are large for a long plan and
/// uninformative when equal: a reader wants to know *which* of the three
/// differed before reading a diff of all of them. The outcome — the part
/// INV-5's name is about — is diffed in full.
fn difference(steps: &[Step], arrangement: Arrangement, sync: &Run, asynchronous: &Run) -> String {
    let outcome = if sync.outcome == asynchronous.outcome {
        "outcomes equal".to_owned()
    } else {
        format!(
            "\n{}",
            Comparison::new(&sync.outcome, &asynchronous.outcome)
        )
    };
    format!(
        "plan={:?} arrangement={arrangement:?}\n{outcome}\nexecuted: sync={:?} \
         async={:?}\nreadings: sync={:?} async={:?}",
        kinds(steps),
        sync.executed,
        asynchronous.executed,
        sync.readings,
        asynchronous.readings,
    )
}

/// The kinds of a generated plan, for a failure message.
fn kinds(steps: &[Step]) -> Vec<Kind> { steps.iter().map(|step| step.kind).collect() }

/// The negative control: the comparison can report a difference.
///
/// INV-5 asserts that two values are equal, which is worth nothing unless
/// inequality is reachable. This is the `cargo-mutants` control's in-suite
/// companion, and it exists because the two are not interchangeable: mutants
/// probe the *driver*, and this probes the *comparison*. A mutant that no plan
/// in the generator's domain can reach would escape the first and not the
/// second.
///
/// The mutation is a recorded source line, which the invariant's own prose does
/// not enumerate — it is the drift a status-only projection would miss, and one
/// a real driver could produce by carrying the wrong invocation's source.
#[test]
fn the_comparison_rejects_a_mutated_outcome() {
    let steps = super::plan(&[Kind::Skip, Kind::Pass]);
    let sync = run_case(&steps, Arrangement::OneProbe);
    let asynchronous = run_case_async(&steps, Arrangement::OneProbe);

    assert!(
        sync == asynchronous,
        "the control's own plan must first agree, or it observes nothing about disagreement",
    );

    let mut shifted = steps.clone();
    let first = shifted
        .first_mut()
        .expect("the control's plan has two invocations");
    first.line += 1;

    let moved = run_case(&shifted, Arrangement::OneProbe);

    assert!(
        sync != moved,
        "two runs whose plans differ in a recorded source line compared equal, so the comparison \
         INV-5 rests on is not comparing the whole value. sync={} moved={}",
        sync.describe(&steps),
        moved.describe(&shifted),
    );
}

/// A `Run`'s outcome is reachable, so the comparison above is not vacuous.
///
/// The control compares runs built from two different plans; this pins the
/// intermediate fact that made it meaningful, that a run's outcome carries the
/// source its plan gave it. If sources stopped being recorded, the control's
/// mutation would become a no-op and it would pass for the wrong reason.
#[test]
fn a_run_records_the_source_its_plan_gave() {
    let steps = super::plan(&[Kind::Pass]);
    let run = run_case(&steps, Arrangement::NoProbe);
    let recorded: Option<u32> = run
        .outcome
        .steps()
        .first()
        .and_then(|record| record.source())
        .map(rstest_bdd::runner::SourceLocation::line);

    assert_eq!(
        recorded,
        Some(steps.first().map_or(0, |step| step.line)),
        "{}",
        run.describe(&steps),
    );
}

/// The async leg's outcome is reachable at all, and is not empty.
///
/// A property that passed because both runners returned the same *empty*
/// outcome would be comparing nothing to nothing. This pins that each runner
/// recorded every invocation and that the plan's steps actually ran, so the
/// equality INV-5 asserts is over real contents. It also pins the async leg's
/// poll count at one for a plan with a value-returning and an observing step,
/// which is the premise [`run_case_async`]'s bound rests on.
#[test]
fn both_runners_record_every_invocation() {
    let steps = super::plan(&[Kind::Pass, Kind::ReturnValue, Kind::Observe]);
    for (label, run) in [
        ("sync", run_case(&steps, Arrangement::OneProbe)),
        ("async", run_case_async(&steps, Arrangement::OneProbe)),
    ] {
        assert_eq!(
            run.outcome.steps().len(),
            steps.len(),
            "({label}) one record per invocation; {}",
            run.describe(&steps),
        );
        assert_eq!(
            run.executed,
            vec![0, 1, 2],
            "({label}) every invocation's handler must have run, in order, or there is nothing \
             for the two runners to agree or disagree about; {}",
            run.describe(&steps),
        );
    }
}
