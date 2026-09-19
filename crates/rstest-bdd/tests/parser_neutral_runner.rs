//! The plan's two behavioural scenarios for parser-neutral execution.
//!
//! The steps below build a plan through the new API and run it through
//! `run_scenario`, while the scenarios *themselves* are executed by the
//! existing, unmigrated macro path. That asymmetry is deliberate and it is what
//! keeps the test honest: if the new runner were used to run its own test, a
//! green result would say only that the runner agrees with itself.
//!
//! # Why the third scenario is not here
//!
//! The plan's specification lists a third scenario, "The asynchronous runner
//! agrees with the synchronous runner", whose `When` is "the plan is executed
//! through both runners". `run_scenario_async` does not exist until EP-M3, so
//! the scenario cannot be bound: its step would not compile. Cutting it is
//! D14's own rule rather than a new decision — an executable scenario may only
//! assert observable behaviour of code that exists. It lands with EP-M3,
//! alongside its siblings in `runner_sequence_props.rs`.
//!
//! # State between steps
//!
//! The plan is built up by three steps and consumed by a fourth, so it is held
//! in a `RefCell<Option<ScenarioPlan>>` fixture rather than passed between step
//! functions — the state is a Rust value this suite constructs, not a step
//! parameter, and threading it through the parameter system would make the
//! `Given` steps describe a context they do not own.

use std::{cell::RefCell, panic::AssertUnwindSafe};

use rstest::fixture;
use rstest_bdd::{
    StepContext,
    StepKeyword,
    runner::{
        ScenarioOutcome,
        ScenarioPlan,
        ScenarioPlanBuilder,
        ScenarioScope,
        ScenarioStatus,
        StepStatus,
        run_scenario,
    },
};
use rstest_bdd_macros::{given, scenario, then, when};

/// The plan under construction, and the outcome once it has been run.
#[derive(Default)]
struct Bench {
    /// Built by the `Given` steps, taken by the `When` step.
    plan: Option<ScenarioPlan>,
    /// Filled by the `When` step, read by the `Then` steps.
    outcome: Option<ScenarioOutcome>,
    /// Set if running the plan unwound, which is the one thing INV-17 forbids.
    unwound: bool,
}

#[rstest_bdd_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn bench() -> RefCell<Bench> { RefCell::new(Bench::default()) }

/// Three registered steps, one per outcome the two scenarios need.
///
/// The patterns are spelled to be unmistakably this suite's, because the
/// registry is process-global and every integration binary in this crate shares
/// it: a text another suite might also register is a future duplicate.
#[given("a parser-neutral bench step passes")]
fn a_bench_step_passes() {}

#[given("a parser-neutral bench step skips")]
fn a_bench_step_skips() {
    rstest_bdd::skip!("the bench asked for a skip");
}

#[then("a parser-neutral bench step fails")]
fn a_bench_step_fails() {
    assert_eq!(1, 0, "deliberate failure from a parser-neutral bench step");
}

/// Map the feature's readable role names onto those three registered steps.
fn step_text(role: &str) -> &'static str {
    match role {
        "passing" => "a parser-neutral bench step passes",
        "skipping" => "a parser-neutral bench step skips",
        "failing" => "a parser-neutral bench step fails",
        other => panic!("the feature must name a role this suite registers; got `{other}`"),
    }
}

#[given("a plan named {name:string} sourced from {source:string}")]
fn a_plan_named(bench: &RefCell<Bench>, name: String, source: String) {
    let mut bench = bench.borrow_mut();
    bench.plan = Some(ScenarioPlanBuilder::new(name, source).at_line(1).build());
}

#[given("the plan has a {role} step at line {line:u32}")]
fn the_plan_has_a_step_at_line(bench: &RefCell<Bench>, role: String, line: u32) {
    let mut bench = bench.borrow_mut();
    let plan = bench
        .plan
        .take()
        .unwrap_or_else(|| panic!("`Given the plan has ...` must follow `Given a plan named ...`"));
    // `step_at` consumes and returns the builder, and a built plan cannot be
    // extended, so each step re-opens the plan through a fresh builder seeded
    // from the one that exists. The seed is the plan's own fields, read back
    // through its accessors, so this cannot drift from what was built.
    let mut builder = ScenarioPlanBuilder::new(plan.name().to_owned(), plan.source().to_owned())
        .allow_skipped(plan.allow_skipped());
    if let Some(source_line) = plan.source_line() {
        builder = builder.at_line(source_line);
    }
    for existing in plan.steps() {
        builder = builder.step(existing.clone());
    }
    bench.plan = Some(
        builder
            .step_at(StepKeyword::Given, step_text(&role), line)
            .build(),
    );
}

#[when("the plan is executed synchronously")]
fn the_plan_is_executed_synchronously(bench: &RefCell<Bench>) {
    let mut bench = bench.borrow_mut();
    let plan = bench
        .plan
        .take()
        .unwrap_or_else(|| panic!("the scenario must build a plan before executing it"));

    // The `catch_unwind` is the point of the step rather than scaffolding.
    // INV-17 claims a step panic is *returned*; a runner that let one unwind
    // would take this test binary down with it, and a `#[should_panic]` would
    // be satisfied by exactly that failure mode. Catching it here turns "did
    // not unwind" into something the next `Then` can assert, and leaves the
    // harness alive to report it.
    let mut ctx = StepContext::default();
    let scope = ScenarioScope::new(&mut ctx).with_skip_policy(false);
    let caught = std::panic::catch_unwind(AssertUnwindSafe(|| run_scenario(&plan, scope)));
    bench.unwound = caught.is_err();
    bench.outcome = caught.ok();
}

/// The outcome the `When` step produced, or a report of what went wrong.
fn outcome(bench: &RefCell<Bench>) -> ScenarioOutcome {
    let bench = bench.borrow();
    assert!(
        !bench.unwound,
        "running the plan unwound; the runner must return a failure instead",
    );
    bench
        .outcome
        .clone()
        .unwrap_or_else(|| panic!("the `When` step must have produced an outcome"))
}

#[then("the outcome is skipped at step {index:usize}")]
fn the_outcome_is_skipped_at_step(bench: &RefCell<Bench>, index: usize) {
    let outcome = outcome(bench);
    assert_eq!(
        outcome.status(),
        ScenarioStatus::Skipped,
        "the plan's second step asks to be skipped, so the run is a skip",
    );
    let skip = outcome
        .skip()
        .unwrap_or_else(|| panic!("a skipped run carries a skip record: {outcome:?}"));
    assert_eq!(
        skip.at(),
        index,
        "the skip is recorded where it was raised, not where the plan ends",
    );
}

#[then("step {index:usize} is recorded as bypassed")]
fn step_is_recorded_as_bypassed(bench: &RefCell<Bench>, index: usize) {
    let outcome = outcome(bench);
    assert_eq!(
        outcome
            .steps()
            .get(index)
            .map(rstest_bdd::runner::StepOutcome::status),
        Some(StepStatus::Bypassed),
        "the step after the skip must be reported, not omitted; a frontend's report is built by \
         walking `steps()`",
    );
}

#[then("every recorded step reports its supplied source line")]
fn every_step_reports_its_source_line(bench: &RefCell<Bench>) {
    let outcome = outcome(bench);
    let lines: Vec<Option<u32>> = outcome
        .steps()
        .iter()
        .map(|step| step.source().map(rstest_bdd::runner::SourceLocation::line))
        .collect();
    assert_eq!(
        lines,
        vec![Some(12), Some(13), Some(14)],
        "each record carries the line its own invocation was built with — the feature supplies \
         three distinct ones so a runner that copied the plan's line, or renumbered from zero, \
         would fail here",
    );
}

#[then("the outcome is failed at step {index:usize}")]
fn the_outcome_is_failed_at_step(bench: &RefCell<Bench>, index: usize) {
    let outcome = outcome(bench);
    assert_eq!(
        outcome.status(),
        ScenarioStatus::Failed,
        "a step whose body panics ends the run as a failure",
    );
    assert_eq!(
        outcome
            .failure()
            .map(rstest_bdd::runner::ScenarioFailure::site),
        Some(rstest_bdd::runner::FailureSite::Step(index)),
        "the failure's site is the invocation that ended the run",
    );
}

#[then("no panic was raised")]
fn no_panic_was_raised(bench: &RefCell<Bench>) {
    assert!(
        !bench.borrow().unwound,
        "the runner returned an outcome, so the step's panic did not reach the harness; a runner \
         that re-threw would be caught by the `When` step",
    );
}

#[then("folding the outcome for the harness yields an error")]
fn folding_yields_an_error(bench: &RefCell<Bench>) {
    let outcome = outcome(bench);
    assert!(
        outcome.into_harness_result().is_err(),
        "the fold is what turns an outcome into a test result, and a failed run must not fold to \
         a pass",
    );
}

#[scenario(
    path = "tests/features/parser_neutral_runner.feature",
    name = "A plan from a Markdown source records every step"
)]
fn a_plan_from_a_markdown_source_records_every_step(#[from(bench)] _bench: RefCell<Bench>) {}

#[scenario(
    path = "tests/features/parser_neutral_runner.feature",
    name = "A failing step returns an outcome rather than panicking"
)]
fn a_failing_step_returns_an_outcome_rather_than_panicking(#[from(bench)] _bench: RefCell<Bench>) {}
