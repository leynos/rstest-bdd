//! The plan's three behavioural scenarios for parser-neutral execution.
//!
//! The steps below build a plan through the new API and run it through
//! `run_scenario`, while the scenarios *themselves* are executed by the
//! existing, unmigrated macro path. That asymmetry is deliberate and it is what
//! keeps the test honest: if the new runner were used to run its own test, a
//! green result would say only that the runner agrees with itself.
//!
//! # The third scenario and the second runner
//!
//! "The asynchronous runner agrees with the synchronous runner" landed with
//! EP-M3, alongside its siblings in `runner_sequence_props.rs`, because it
//! could not be bound before `run_scenario_async` existed: its `When` step
//! would not have compiled. It asserts equality of the *whole* outcome rather
//! than of a status or a step count, and it is deliberately the weaker of the
//! two INV-5 artefacts — the property suite covers generated plans, this one
//! covers a plan a human named, including a returning step whose value fate
//! must match. Both are kept because the property is only as good as its
//! generator, and a reader checking "does the async runner agree" should be
//! able to read one plan and see for themselves.
//!
//! # Why the async arm needs a runtime here but not in `cancel.rs`
//!
//! This suite's steps are ordinary `Sync`-mode registrations, so
//! `execute_step_async` runs their handlers synchronously and never touches
//! Tokio — the future it returns is ready on its first poll. But
//! `run_scenario_async` is still an `async fn`, and this step is a synchronous
//! `fn` that has to produce an outcome from it, so something must drive the
//! future to completion. A current-thread runtime is the honest tool: it is
//! what a caller would really use, and it is what makes this scenario evidence
//! about the *shipping* path rather than about a hand-rolled poll loop. The
//! poll loop is `cancel.rs`'s subject, and it lives there because cancellation
//! is only observable if the test owns the polls.
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
        run_scenario_async,
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
    /// Filled by the equivalence scenario's `When`, read by its `Then`.
    ///
    /// The two runners' results are kept side by side rather than compared
    /// inside the step, so a failure reports both outcomes through the
    /// assertion's own diff instead of a hand-built message.
    async_outcome: Option<ScenarioOutcome>,
    /// Set if running the plan unwound, which is the one thing INV-17 forbids.
    unwound: bool,
}

#[rstest_bdd_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn bench() -> RefCell<Bench> { RefCell::new(Bench::default()) }

/// Four registered steps, one per role the three scenarios need.
///
/// The patterns are spelled to be unmistakably this suite's, because the
/// registry is process-global and every integration binary in this crate shares
/// it: a text another suite might also register is a future duplicate.
///
/// Each is registered under the keyword `step_text` maps its role to, and
/// `resolve_step` filters on keyword equality — so the mapping is not a
/// convenience but the thing that makes each invocation resolve at all. The
/// returning step is under `When` for that reason, and it is the one that makes
/// the equivalence scenario more than a status comparison: a value whose
/// `InsertOutcome` differed between the runners would give two outcomes that
/// agree on every status and disagree on `value_insertion`.
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

/// A step that returns a value matching no fixture in the bench's context.
///
/// `NoMatch` is the fate both runners must report. Deliberately *not* a value
/// the context can hold: an `Inserted` fate would depend on a fixture cell this
/// suite would have to register, and the equivalence claim is stronger when the
/// fate under comparison is the one that arises from the plan alone.
#[when("a parser-neutral bench step returns a value")]
fn a_bench_step_returns_a_value() -> BenchValue { BenchValue }

/// The returned value's type, deliberately unlike anything the bench inserts.
#[derive(Debug)]
struct BenchValue;

/// Map the feature's readable role names onto the registered steps' text.
fn step_text(role: &str) -> (&'static str, StepKeyword) {
    match role {
        "passing" => ("a parser-neutral bench step passes", StepKeyword::Given),
        "skipping" => ("a parser-neutral bench step skips", StepKeyword::Given),
        "failing" => ("a parser-neutral bench step fails", StepKeyword::Then),
        "returning" => (
            "a parser-neutral bench step returns a value",
            StepKeyword::When,
        ),
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
    let Some(plan) = bench.plan.take() else {
        panic!("`Given the plan has ...` must follow `Given a plan named ...`");
    };
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
    let (text, keyword) = step_text(&role);
    bench.plan = Some(builder.step_at(keyword, text, line).build());
}

#[when("the plan is executed synchronously")]
fn the_plan_is_executed_synchronously(bench: &RefCell<Bench>) {
    let mut bench = bench.borrow_mut();
    let Some(plan) = bench.plan.take() else {
        panic!("the scenario must build a plan before executing it");
    };

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

/// Run the plan through both runners and keep both outcomes.
///
/// The synchronous arm runs first and the asynchronous one second, each
/// against a *fresh* context. That is not incidental: `ScenarioScope`
/// deliberately does not support reusing one context across runs (see its
/// documentation — a reused context gives partial isolation, which is worse
/// than none), and reusing one here would make the two runs differ for a reason
/// that has nothing to do with which runner executed them.
///
/// Both arms are inside one `catch_unwind`, so an unwind in either fails the
/// same assertion. A runner that threw would otherwise take the test binary
/// down and produce a failure with no attribution.
#[when("the plan is executed through both runners")]
fn the_plan_is_executed_through_both_runners(bench: &RefCell<Bench>) {
    let mut bench = bench.borrow_mut();
    let Some(plan) = bench.plan.take() else {
        panic!("the scenario must build a plan before executing it");
    };

    let caught = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let mut sync_ctx = StepContext::default();
        let sync_outcome =
            run_scenario(&plan, ScenarioScope::new(&mut sync_ctx).with_skip_policy(false));

        // A current-thread runtime with time and I/O *disabled*: the steps this
        // suite registers are `Sync`-mode, so no timer or reactor is ever
        // touched, and leaving them off means a future edit that introduced one
        // would fail here loudly instead of here mysteriously.
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("a current-thread runtime builds");
        let mut async_ctx = StepContext::default();
        let async_outcome = runtime.block_on(run_scenario_async(
            &plan,
            ScenarioScope::new(&mut async_ctx).with_skip_policy(false),
        ));

        (sync_outcome, async_outcome)
    }));

    bench.unwound = caught.is_err();
    if let Ok((sync_outcome, async_outcome)) = caught {
        bench.outcome = Some(sync_outcome);
        bench.async_outcome = Some(async_outcome);
    }
}

/// The asynchronous outcome, or a report of what went wrong.
fn async_outcome(bench: &RefCell<Bench>) -> ScenarioOutcome {
    let bench = bench.borrow();
    assert!(
        !bench.unwound,
        "running the plan unwound; the runner must return a failure instead",
    );
    let Some(outcome) = bench.async_outcome.clone() else {
        panic!("the `When` step must have produced an asynchronous outcome");
    };
    outcome
}

/// The outcome the `When` step produced, or a report of what went wrong.
fn outcome(bench: &RefCell<Bench>) -> ScenarioOutcome {
    let bench = bench.borrow();
    assert!(
        !bench.unwound,
        "running the plan unwound; the runner must return a failure instead",
    );
    let Some(outcome) = bench.outcome.clone() else {
        panic!("the `When` step must have produced an outcome");
    };
    outcome
}

#[then("the outcome is skipped at step {index:usize}")]
fn the_outcome_is_skipped_at_step(bench: &RefCell<Bench>, index: usize) {
    let outcome = outcome(bench);
    assert_eq!(
        outcome.status(),
        ScenarioStatus::Skipped,
        "the plan's second step asks to be skipped, so the run is a skip",
    );
    let Some(skip) = outcome.skip() else {
        panic!("a skipped run carries a skip record: {outcome:?}");
    };
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

#[then("the two outcomes are equal")]
fn the_two_outcomes_are_equal(bench: &RefCell<Bench>) {
    // Both are taken before the assertion so that a mismatch reports two
    // outcomes rather than one and a panic.
    let sync = outcome(bench);
    let asynchronous = async_outcome(bench);
    assert_eq!(
        sync, asynchronous,
        "INV-5: for a plan whose every step is registered in `StepExecutionMode::Both`, the two \
         runners must produce equal outcomes — compared whole, because a projection chosen by \
         this test could omit exactly the field that differs",
    );
}

#[then("neither runner unwound")]
fn neither_runner_unwound(bench: &RefCell<Bench>) {
    assert!(
        !bench.borrow().unwound,
        "both runners returned an outcome, so neither re-threw; a runner that unwound would have \
         been caught by the `When` step",
    );
}

#[scenario(
    path = "tests/features/parser_neutral_runner.feature",
    name = "A failing step returns an outcome rather than panicking"
)]
fn a_failing_step_returns_an_outcome_rather_than_panicking(#[from(bench)] _bench: RefCell<Bench>) {}

#[scenario(
    path = "tests/features/parser_neutral_runner.feature",
    name = "The asynchronous runner agrees with the synchronous runner"
)]
fn the_asynchronous_runner_agrees_with_the_synchronous_runner(
    #[from(bench)] _bench: RefCell<Bench>,
) {
}
