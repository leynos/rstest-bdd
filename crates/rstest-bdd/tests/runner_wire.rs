//! INV-1, INV-12, INV-13: the runner reaches the registry and records the run.
//!
//! Everything else in `src/runner`'s tests checks a type or a fold in
//! isolation. This file checks the thing those cannot: that `run_scenario`
//! actually reaches the step registry. A runner that returned a well-formed
//! outcome describing a run it never performed would satisfy every other test
//! in the crate and be useless, so the load-bearing assertion here is the side
//! effect on a fixture, not the shape of the returned outcome.
//!
//! The property under test is not "a passing scenario passes" — a stub could
//! satisfy that. It is that the run is *observable*: a step that mutates a
//! fixture is seen to have run, a step that does not resolve is seen to have
//! failed, and the error the outcome carries is the registry's own
//! `StepNotFound`, not a synthesised substitute. Those three together are what
//! a fake cannot fake.
//!
//! # Why this is an integration test
//!
//! `run_scenario` is public API and these are its first end-to-end callers, so
//! an integration test is the honest home for them. It is also the only
//! *possible* home today: `crates/rstest-bdd/src/registry/introspection.rs`
//! deliberately registers the same pattern twice to exercise
//! `duplicate_steps`, and the first lookup in a process builds `STEP_MAP`,
//! whose duplicate `assert!` fires on it. The unit-test binary therefore cannot
//! resolve any step at all, with or without the runner.
//!
//! # Two conventions this file had to learn
//!
//! Steps receive fixtures and nothing else. There is no `&mut StepContext`
//! parameter: the macro has no special case for one, so writing it declares a
//! *fixture requirement* named `ctx` and the step then fails validation with
//! `MissingFixtures { missing: ["ctx"] }`. The driver owns the context, and
//! this test deliberately never names it after the plan is built.
//!
//! A step parameter that is syntactically a reference (here `&Cell<u32>`) is
//! served from the *mutable* fixture storage, not from a shared reference, and
//! the entry's `TypeId` is the referent's. So the fixture is registered with
//! [`StepContext::insert_owned`] rather than [`StepContext::insert`].

use std::{
    any::Any,
    cell::{Cell, RefCell},
};

use rstest_bdd::{
    ExecutionError, StepContext, StepKeyword,
    runner::{
        ScenarioPlanBuilder, ScenarioScope, ScenarioStatus, StepOutcome, StepStatus, run_scenario,
    },
};
use rstest_bdd_macros::{given, when};

/// The counter the steps below mutate, so a test can see the run happen.
const COUNTER: &str = "counter";

// Interior mutability, so the step takes a shared reference and the test can
// still read what the step did.
#[given("a counter starts at zero")]
fn a_counter_starts_at_zero(counter: &Cell<u32>) {
    counter.set(0);
}

#[when("the counter is incremented twice")]
fn the_counter_is_incremented_twice(counter: &Cell<u32>) {
    counter.set(counter.get() + 2);
}

/// A fresh counter cell, still holding its sentinel value.
///
/// The caller owns the cell and the context borrows it, which is the real
/// arrangement: a fixture outlives the run over it.
fn counter_cell() -> RefCell<Box<dyn Any>> {
    StepContext::owned_cell(Cell::new(u32::MAX))
}

/// The context a run borrows: the counter cell, registered under its name.
fn context_for(cell: &RefCell<Box<dyn Any>>) -> StepContext<'_> {
    let mut ctx = StepContext::default();
    ctx.insert_owned::<Cell<u32>>(COUNTER, cell);
    ctx
}

/// Read the counter back out of its cell, consuming the cell.
fn counter_value(cell: RefCell<Box<dyn Any>>) -> u32 {
    cell.into_inner()
        .downcast::<Cell<u32>>()
        .expect("the cell holds a Cell<u32>")
        .get()
}

/// The run reached the registry and its steps mutated the caller's fixture.
///
/// This is the assertion a stub cannot satisfy. The starting value is
/// `u32::MAX` rather than zero, so a run that silently did nothing is
/// distinguishable from one that reset the cell.
#[test]
fn a_run_executes_its_steps_against_the_context() {
    let cell = counter_cell();
    let mut ctx = context_for(&cell);
    let plan = ScenarioPlanBuilder::new("Counting", "notes/counting.md")
        .step_at(StepKeyword::Given, "a counter starts at zero", 3)
        .step_at(StepKeyword::When, "the counter is incremented twice", 4)
        .step_at(StepKeyword::When, "the counter is incremented twice", 5)
        .build();

    let outcome = {
        let scope = ScenarioScope::new(&mut ctx);
        run_scenario(&plan, scope)
    };

    assert_eq!(
        outcome.status(),
        ScenarioStatus::Passed,
        "all three registered steps must run: {outcome:?}",
    );
    assert_eq!(
        counter_value(cell),
        4,
        "each increment step ran once, so the cell left `u32::MAX` behind",
    );
    assert!(
        outcome
            .steps()
            .iter()
            .all(|step| step.status() == StepStatus::Passed),
    );
}

/// A plan step that resolves to no definition fails with the registry's own
/// error, carried verbatim.
///
/// This is what proves the driver actually called `execute_step` with the
/// plan's step: the error is the registry's `StepNotFound`, carrying the text
/// and scenario name the plan supplied. A driver that invented a failure, or
/// that reported the wrong index, is caught here.
#[test]
fn an_unresolvable_step_fails_with_the_registrys_own_error() {
    let cell = counter_cell();
    let mut ctx = context_for(&cell);
    let plan = ScenarioPlanBuilder::new("Counting", "notes/counting.md")
        .step_at(StepKeyword::Given, "a counter starts at zero", 3)
        .step_at(StepKeyword::Then, "a step nobody wrote", 4)
        .build();

    let outcome = {
        let scope = ScenarioScope::new(&mut ctx);
        run_scenario(&plan, scope)
    };

    assert_eq!(outcome.status(), ScenarioStatus::Failed);
    let Some(ExecutionError::StepNotFound {
        index,
        text,
        scenario_name,
        ..
    }) = outcome.steps().get(1).and_then(StepOutcome::error)
    else {
        panic!("the second step must carry the registry's error: {outcome:?}");
    };
    assert_eq!(*index, 1, "the error names the invocation it happened at");
    assert_eq!(text, "a step nobody wrote");
    assert_eq!(
        scenario_name, "Counting",
        "the scenario name reaches the registry's diagnostics",
    );
}

/// A failing step stops the run and bypasses the rest (INV-1).
///
/// The failure is at the first invocation, so this covers the termination and
/// completeness halves of INV-1 together: the second invocation must be
/// recorded, and must not have been executed. The fixture is the witness for
/// the second half — a runner that ran it anyway would have left `0` behind
/// rather than `u32::MAX`.
#[test]
fn a_failing_step_stops_the_run_and_bypasses_the_rest() {
    let cell = counter_cell();
    let mut ctx = context_for(&cell);
    let plan = ScenarioPlanBuilder::new("Counting", "notes/counting.md")
        .step_at(StepKeyword::Then, "a step nobody wrote", 3)
        .step_at(StepKeyword::Given, "a counter starts at zero", 4)
        .build();

    let outcome = {
        let scope = ScenarioScope::new(&mut ctx);
        run_scenario(&plan, scope)
    };

    assert_eq!(outcome.status(), ScenarioStatus::Failed);
    assert_eq!(outcome.steps().len(), 2, "every invocation is recorded");
    assert_eq!(
        outcome.steps().get(1).map(StepOutcome::status),
        Some(StepStatus::Bypassed),
    );
    assert_eq!(
        counter_value(cell),
        u32::MAX,
        "the bypassed step must not have run: it would have zeroed the cell",
    );
}

/// A run over an empty plan is INV-13's case end to end.
///
/// The builder is handed no steps, so the runner executes nothing, and the
/// outcome says `Passed` while the fold refuses it. Asserting both here is
/// what keeps the two halves — `assemble` reporting success for a vacuous run
/// and `into_harness_result` rejecting it — from drifting apart.
#[test]
fn a_plan_with_no_steps_does_not_fold_to_a_clean_pass() {
    let cell = counter_cell();
    let mut ctx = context_for(&cell);
    let plan = ScenarioPlanBuilder::new("Empty", "notes/empty.md").build();

    let outcome = {
        let scope = ScenarioScope::new(&mut ctx);
        run_scenario(&plan, scope)
    };

    assert_eq!(outcome.status(), ScenarioStatus::Passed);
    assert!(outcome.steps().is_empty());
    assert!(outcome.into_harness_result().is_err());
}
