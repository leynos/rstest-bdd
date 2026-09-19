//! The run harness, and the predicates a run's result answers.
//!
//! Split out of `sequence/mod.rs` to keep both files inside the repository's
//! 400-line cap. The vocabulary — [`Kind`](super::Kind), [`Step`],
//! [`Arrangement`], [`Reading`] — stays in the parent because the generator and
//! the failure messages speak it too; this file holds what is *done* with that
//! vocabulary: building the context an arrangement describes, driving the plan
//! through `run_scenario`, and reading the result back.
//!
//! # Why the predicates live beside the harness
//!
//! [`Run`] is what one case produced, and every predicate here is a pure
//! function of one. Keeping them in the same file means a property in
//! `runner_sequence_props.rs` can state an invariant that is *decided* here,
//! and the negative controls there can hand these predicates a run that
//! violates it. A predicate that had quietly stopped being able to report a
//! violation would otherwise be a hole in a file nobody was looking at.

use std::any::Any;
use std::cell::RefCell;

use rstest_bdd::StepContext;
use rstest_bdd::runner::{
    FailureKind, ScenarioOutcome, ScenarioPlan, ScenarioPlanBuilder, ScenarioScope, StepStatus,
    ValueFate, run_scenario,
};

use super::{Arrangement, Reading, SENTINEL, Step, executed, observed, reset_logs};

/// The result of running one generated case.
#[derive(Debug, Clone)]
pub(crate) struct Run {
    /// What the driver returned.
    pub(crate) outcome: ScenarioOutcome,
    /// The invocation indices whose handlers ran, in execution order, appended
    /// by the handlers themselves rather than by the driver.
    ///
    /// This is the log INV-1 is checked against. Reading it back through the
    /// steps rather than through `outcome.steps()` is what makes that check
    /// independent: `steps()` records what the driver *says* it did, and a
    /// driver that ran a bypassed invocation and then recorded it as
    /// `Bypassed` would satisfy every assertion drawn from `steps()` alone.
    pub(crate) executed: Vec<usize>,
    /// What each observer read, in the order the observers ran.
    pub(crate) readings: Vec<Reading>,
}

impl Run {
    /// The index beyond which nothing may run: the terminal invocation's.
    ///
    /// Taken from the outcome rather than from the generated plan, so a driver
    /// that stopped somewhere the generator did not intend is described by
    /// where it actually stopped. `None` for a plan that ran to the end, which
    /// is also what an empty plan gives.
    pub(crate) fn terminal_index(&self) -> Option<usize> {
        self.outcome.steps().iter().position(|record| {
            !matches!(record.status(), StepStatus::Passed | StepStatus::Bypassed)
        })
    }

    /// The first invocation past the terminal index that the executor ran.
    ///
    /// INV-1 as a predicate rather than as an assertion, so the negative
    /// control in `runner_sequence_props.rs` can hand it a run that *does*
    /// exceed its terminal and require it to say so. An assertion that nothing
    /// is ever fed a counter-example carries no evidence on its own.
    pub(crate) fn first_exceeding(&self) -> Option<usize> {
        let terminal = self.terminal_index()?;
        self.executed.iter().copied().find(|&index| index > terminal)
    }

    /// Whether any invocation past the terminal index was executed.
    pub(crate) fn terminal_exceeded(&self) -> bool { self.first_exceeding().is_some() }

    /// The first visibility violation, if any.
    ///
    /// INV-3 says a value returned by invocation `i` reaches every `j > i` and
    /// no `j <= i`. A reading names its producer, so a reading of `Some(i)` by
    /// an observer at `j` violates the second clause exactly when `i >= j`. A
    /// reading of the [`SENTINEL`] names no producer and cannot violate it; a
    /// reading of `None` means the name did not resolve, which under an
    /// arrangement that registers a probe can only happen before any producer
    /// has run, so it too names no producer.
    pub(crate) fn visibility_violation(&self) -> Option<String> {
        self.readings.iter().find_map(|reading| {
            let seen = reading.value?;
            (seen != SENTINEL && seen >= reading.observer).then(|| {
                format!(
                    "observer {} read producer {seen}, so a value reached an invocation at or \
                     before its producer",
                    reading.observer
                )
            })
        })
    }

    /// The recorded fate of each value-returning invocation, in plan order.
    ///
    /// `None` where the driver recorded no fate at all, which is the shape
    /// INV-12 forbids for an invocation that ran and succeeded.
    pub(crate) fn fates(&self, plan: &[Step]) -> Vec<(usize, Option<ValueFate>)> {
        plan.iter()
            .enumerate()
            .filter(|(_, step)| step.kind.returns_a_value())
            .map(|(index, _)| {
                let fate = self
                    .outcome
                    .steps()
                    .get(index)
                    .and_then(rstest_bdd::runner::StepOutcome::value_insertion);
                (index, fate)
            })
            .collect()
    }

    /// The classification of every record that failed, for the classification.
    pub(crate) fn failure_kinds(&self) -> Vec<FailureKind> {
        self.outcome
            .steps()
            .iter()
            .filter_map(rstest_bdd::runner::StepOutcome::failure_kind)
            .collect()
    }

    /// Render a compact description for a failing assertion.
    pub(crate) fn describe(&self, plan: &[Step]) -> String {
        let texts: Vec<String> = plan
            .iter()
            .enumerate()
            .map(|(index, step)| format!("{:?}({index})", step.kind))
            .collect();
        let statuses: Vec<Option<StepStatus>> = self
            .outcome
            .steps()
            .iter()
            .map(|record| Some(record.status()))
            .collect();
        // The site alone does not say *why* a step failed, and a failure inside
        // the macro-generated argument binding is indistinguishable from one in
        // the handler without it. So the error is rendered too: a classification
        // mismatch reported without it would name the symptom and hide the
        // cause.
        let failure = self.outcome.failure().map_or_else(
            || "none".to_owned(),
            |failure| format!("{:?} {:?}", failure.site(), failure.error()),
        );
        format!(
            "status={:?} failure={failure} recorded={statuses:?} executed={:?} readings={:?} \
             plan={texts:?}",
            self.outcome.status(),
            self.executed,
            self.readings,
        )
    }
}

/// A fixture cell holding a probe, boxed so `insert_owned` can take it.
type ProbeCell = RefCell<Box<dyn Any>>;

/// Build the context an arrangement describes.
///
/// Registering the fixture as `insert_owned::<Probe>` and not as
/// `insert_owned::<RefCell<Box<dyn Any>>>` is load-bearing: `insert_value`
/// compares the returned value's `TypeId` against each entry's stored
/// `type_id`, and `insert_owned::<T>` stores `TypeId::of::<T>()` — the
/// *parameter's* type, never the boxed referent's. Naming the cell type here
/// would store the wrong id and turn every generated case into `NoMatch`, which
/// is precisely the false green INV-12 exists to catch.
pub(crate) fn context_for(arrangement: Arrangement, cells: &[ProbeCell]) -> StepContext<'_> {
    let names = arrangement.probe_names();
    assert_eq!(
        names.len(),
        cells.len(),
        "({arrangement:?}) an arrangement registers exactly the cells it was given",
    );

    let mut ctx = StepContext::default();
    for (name, cell) in names.iter().zip(cells) {
        ctx.insert_owned::<super::steps::Probe>(name, cell);
    }
    ctx
}

/// The source every generated plan claims.
pub(crate) const SOURCE: &str = "notes/sequence.md";

/// The plan a generated step list describes.
pub(crate) fn plan_for(steps: &[Step]) -> ScenarioPlan {
    steps
        .iter()
        .enumerate()
        .fold(
            ScenarioPlanBuilder::new("Sequence", SOURCE).at_line(1),
            |builder, (index, step)| {
                builder.step_at(step.keyword, step.kind.text(index), step.line)
            },
        )
        .build()
}

/// Run one generated case and collect everything the properties read.
pub(crate) fn run_case(steps: &[Step], arrangement: Arrangement) -> Run {
    reset_logs();
    let plan = plan_for(steps);
    let cells: Vec<ProbeCell> = (0..arrangement.probes())
        .map(|_| StepContext::owned_cell(super::steps::Probe(SENTINEL)))
        .collect();
    let mut ctx = context_for(arrangement, &cells);
    let scope = ScenarioScope::new(&mut ctx).with_skip_policy(false);
    let outcome = run_scenario(&plan, scope);
    Run {
        outcome,
        executed: executed(),
        readings: observed(),
    }
}
