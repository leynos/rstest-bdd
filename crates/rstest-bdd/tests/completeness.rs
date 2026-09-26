//! INV-2 and INV-13: every invocation is recorded, in order, exactly once.
//!
//! INV-2 says `outcome.steps().len()` equals `plan.steps().len()`; that entry
//! `i` carries invocation `i`'s keyword, text, and source; and that entries past
//! the terminal index are `Bypassed`. It is the invariant a frontend's report is
//! built on: a Cucumber-Messages-shaped emitter walks `steps()` and prints one
//! line each, so a truncating runner does not crash it — it silently prints a
//! shorter, happier report. That is why the property is asserted end to end
//! rather than inferred from the engine's own unit tests.
//!
//! INV-13 covers the boundary of the same claim: a plan with no steps at all.
//! The runner reports `Passed` with an empty list, which the fold must refuse,
//! because a dynamic parser can emit an empty plan from a malformed document and
//! "no steps ran, so nothing failed" is the canonical false green.
//!
//! # Why this is an integration test
//!
//! D21: these statements are about invocations that resolve, and the unit-test
//! binary cannot reach the registry (see `runner_wire.rs`).
//!
//! INV-2 claims the accounting holds with the `diagnostics` feature on *and*
//! off, and the two configurations are different compiled artefacts — a runtime
//! test cannot switch features. There is no feature-gated sub-module here; the
//! whole file is simply compiled twice. The second leg is a
//! `--no-default-features` build of this same file, driven by the `test` target
//! in the `Makefile` (`Makefile:119-121`), and that leg is what makes the claim
//! about both configurations rather than just the default one.

use rstest::rstest;
use rstest_bdd::{
    StepContext,
    StepKeyword,
    runner::{
        ScenarioOutcome,
        ScenarioPlan,
        ScenarioPlanBuilder,
        ScenarioScope,
        ScenarioStatus,
        SourceLocation,
        StepOutcome,
        StepStatus,
        run_scenario,
    },
};
use rstest_bdd_macros::{given, then};

/// A step that resolves and passes, so a plan can have a non-terminal prefix.
#[given("a completeness step passes")]
fn a_completeness_step_passes() {}

/// A step that resolves, runs, and fails — a terminal event that is not a skip.
#[then("a completeness step fails")]
fn a_completeness_step_fails() {
    assert_eq!(1, 0, "deliberate failure from a completeness step");
}

/// A step that resolves, runs, and asks to be skipped — the third terminal
/// shape, and the one the `skip_at_the_second_of_four` case needs.
///
/// Registered rather than reached through an unregistered text: an invocation
/// nothing answers to fails as `Undefined`, which is a *failure*, so the case
/// would have exercised the same terminal path as `failure_at_the_first_of_four`
/// while claiming to be about a skip. The module doc says the cases are the
/// three ways a run can end, and that claim needs a step that can actually skip.
#[given("a completeness step skips")]
fn a_completeness_step_skips() {
    rstest_bdd::skip!("the completeness case asked for a skip");
}

/// A plan built from `(keyword, text, line)` triples.
///
/// The lines are distinct and non-sequential so "entry `i` carries invocation
/// `i`'s source" is falsifiable: a runner that copied the plan's own line onto
/// every step, or that renumbered from zero, would satisfy a plan whose lines
/// happened to match the indices.
fn plan(name: &'static str, steps: &[(u32, StepKeyword, &'static str)]) -> ScenarioPlan {
    let mut builder = ScenarioPlanBuilder::new(name, "notes/completeness.md").at_line(99);
    for (line, keyword, text) in steps {
        builder = builder.step_at(*keyword, *text, *line);
    }
    builder.build()
}

/// Run a plan against a fresh context.
fn run(plan: &ScenarioPlan) -> ScenarioOutcome {
    let mut ctx = StepContext::default();
    let scope = ScenarioScope::new(&mut ctx).with_skip_policy(false);
    run_scenario(plan, scope)
}

/// INV-2, exhaustively: for each terminal shape, every entry is accounted for.
///
/// The three cases are the three ways a run can end — nowhere (all passed), at a
/// failure, and at a skip — and each is given trailing invocations so the
/// `Bypassed` tail exists to be checked. The fourth case, the empty plan, is
/// INV-13's and is below.
///
/// Each case carries the status it claims as well as the terminal index, so the
/// names are load-bearing rather than decorative. They were not, and the skip
/// case was wrong because of it: its terminal invocation used a text nothing was
/// registered under, which fails as `Undefined` rather than skipping, so the case
/// exercised the *failure* path while calling itself `Skipped`. A positional-only
/// assertion cannot catch that — both shapes are "reached, not `Bypassed`" — so
/// the status is asserted directly and the step it names is now a real
/// registration.
#[rstest]
#[case::all_pass(
    "AllPass", &[(3, StepKeyword::Given, "a completeness step passes")], 0,
    ScenarioStatus::Passed
)]
#[case::failure_at_the_first_of_four(
    "Failed",
    &[
        (3, StepKeyword::Then, "a completeness step fails"),
        (4, StepKeyword::Given, "a completeness step passes"),
        (5, StepKeyword::Given, "a completeness step passes"),
    ],
    0,
    ScenarioStatus::Failed
)]
#[case::skip_at_the_second_of_four(
    "Skipped",
    &[
        (3, StepKeyword::Given, "a completeness step passes"),
        (4, StepKeyword::Given, "a completeness step skips"),
        (5, StepKeyword::Given, "a completeness step passes"),
        (6, StepKeyword::Given, "a completeness step passes"),
    ],
    1,
    ScenarioStatus::Skipped
)]
fn every_invocation_is_recorded_once_in_order(
    #[case] name: &'static str,
    #[case] steps: &[(u32, StepKeyword, &'static str)],
    #[case] terminal_index: usize,
    #[case] expected_status: ScenarioStatus,
) {
    let plan = plan(name, steps);
    let outcome = run(&plan);

    assert_eq!(
        outcome.status(),
        expected_status,
        "({name}) the case is named for the terminal shape it produces, and that name is checked",
    );
    assert_eq!(
        outcome.steps().len(),
        plan.steps().len(),
        "({name}) completeness: the outcome has one entry per planned invocation",
    );

    for (index, (record, invocation)) in outcome.steps().iter().zip(plan.steps()).enumerate() {
        // Position, not per-step expectation: the prefix either passed or is the
        // terminal event itself, and both are "reached". The two branches assert
        // different things, so neither is written as a comparison of a value
        // with itself.
        if index <= terminal_index {
            assert_ne!(
                record.status(),
                StepStatus::Bypassed,
                "({name}) entry {index} is at or before the terminal index and must have run",
            );
        } else {
            assert_eq!(
                record.status(),
                StepStatus::Bypassed,
                "({name}) entry {index} is past the terminal index and must be bypassed",
            );
        }

        // The identity half. Text and keyword are compared as the plan holds
        // them, so an entry carrying the wrong invocation's data — an off-by-one
        // in the zip, say — fails here.
        assert_eq!(
            record.keyword(),
            invocation.keyword(),
            "({name}) entry {index} carries its own keyword",
        );
        assert_eq!(
            record.text(),
            invocation.text(),
            "({name}) entry {index} carries its own text",
        );
        assert_eq!(
            record.index(),
            index,
            "({name}) entry {index} carries its own index",
        );
        assert_eq!(
            record.source().map(SourceLocation::line),
            Some(invocation.source().map_or_else(
                || panic!("({name}) the builder records every step's source"),
                SourceLocation::line,
            )),
            "({name}) entry {index} carries its own source line",
        );
        assert_eq!(
            record.source().map(SourceLocation::path),
            Some("notes/completeness.md"),
        );
    }
}

/// INV-2's non-vacuity control: the `Bypassed` tail must be non-empty.
///
/// The loop above checks a trailing entry is `Bypassed` only when there is one,
/// so a plan whose terminal event is last would leave the bypassed branch
/// unexercised — and a runner that never emitted a `Bypassed` entry at all would
/// agree with every assertion it makes. The two non-empty cases are split out
/// here so the count is asserted directly, and the case with no tail is asserted
/// to have none rather than merely to have satisfied the loop.
#[rstest]
#[case::a_tail_exists("Failed", &[(3, StepKeyword::Then, "a completeness step fails"), (4, StepKeyword::Given, "a completeness step passes")], 1)]
#[case::no_tail_exists("AllPass", &[(3, StepKeyword::Given, "a completeness step passes")], 0)]
fn the_bypassed_tail_has_the_length_the_terminal_event_implies(
    #[case] name: &'static str,
    #[case] steps: &[(u32, StepKeyword, &'static str)],
    #[case] expected_bypassed: usize,
) {
    let outcome = run(&plan(name, steps));

    let bypassed = outcome
        .steps()
        .iter()
        .filter(|step| step.status() == StepStatus::Bypassed)
        .count();
    assert_eq!(
        bypassed, expected_bypassed,
        "({name}) the tail after the terminal event is exactly {expected_bypassed} long",
    );
}

/// INV-13: an empty plan is `Passed`, empty, and refused by the fold.
///
/// All three assertions are load-bearing together. Dropping the first two would
/// let a runner that reported `Failed` for an empty plan pass, which is a
/// different (and worse) bug — a parser emitting a malformed document would look
/// like a failing test rather than an empty one. Dropping the third would ship
/// the false green. The one-step case below is the control that separates "the
/// fold refuses empty plans" from "the fold refuses everything".
#[test]
fn an_empty_plan_is_a_passed_run_the_fold_refuses() {
    let plan = plan("Empty", &[]);
    let outcome = run(&plan);

    assert_eq!(outcome.status(), ScenarioStatus::Passed);
    assert!(outcome.steps().is_empty());
    assert!(outcome.skip().is_none());
    assert!(outcome.failure().is_none());
    assert!(
        outcome.clone().into_harness_result().is_err(),
        "an empty plan must not fold to a clean pass: {outcome:?}",
    );
}

/// The control for the fold: a one-step passing plan folds to `Ok`.
///
/// Without it, a fold returning `Err` for *every* outcome would satisfy the
/// assertion above. The plan differs from the empty one by exactly one step, so
/// the pair isolates "empty" as the reason.
#[test]
fn a_one_step_passing_plan_folds_to_ok() {
    let outcome = run(&plan(
        "One",
        &[(3, StepKeyword::Given, "a completeness step passes")],
    ));

    assert_eq!(outcome.status(), ScenarioStatus::Passed);
    assert_eq!(outcome.steps().len(), 1);
    assert!(
        outcome.into_harness_result().is_ok(),
        "a plan that ran a step and passed is the fold's happy path",
    );
}

/// The outcome's own accessors agree with the records, on a run with a tail.
///
/// `terminal_source` is the one accessor here that is not a projection of
/// `steps()` in the obvious way, so it is pinned where a tail exists: a runner
/// that returned the *last* invocation's source rather than the terminal one's
/// differs only when there is a tail to be wrong about.
#[test]
fn the_outcome_accessors_agree_with_the_records_on_a_failing_run() {
    let outcome = run(&plan(
        "Failed",
        &[
            (3, StepKeyword::Then, "a completeness step fails"),
            (4, StepKeyword::Given, "a completeness step passes"),
        ],
    ));

    assert_eq!(outcome.status(), ScenarioStatus::Failed);
    let Some(first) = outcome.steps().first() else {
        panic!("the plan has two invocations, so a first record must exist");
    };
    assert_eq!(
        outcome.terminal_source(),
        first.source(),
        "the terminal event is the first step's, so its source is the outcome's",
    );
    assert_eq!(
        outcome
            .failure()
            .map(rstest_bdd::runner::ScenarioFailure::site),
        Some(rstest_bdd::runner::FailureSite::Step(0)),
        "the failure's site is the invocation that ended the run",
    );
    assert_eq!(
        first.failure_kind(),
        Some(rstest_bdd::runner::FailureKind::Panic),
        "a failing `assert_eq!` in a step body is a *panic* by the time the runner sees it: the \
         macro wrapper catches it and builds a `PanicError`. `Assertion` is the label for a \
         handler that *returns* a `StepError`, so a boundary that swallowed the panic and \
         relabelled it as a returned error would fail here",
    );
    let Some(second) = outcome.steps().get(1) else {
        panic!("the second invocation must be recorded even though it never ran");
    };
    assert_eq!(second.status(), StepStatus::Bypassed);
    assert!(
        second.source().is_some(),
        "a bypassed step keeps its source"
    );
    assert!(second.error().is_none(), "a bypassed step has no error");
}

/// The unused-import guard for [`StepOutcome`].
///
/// `StepOutcome` is named in this file only through inference; importing it and
/// never naming it would be a warning under `-D warnings`. Rather than remove
/// the import and rely on inference, the file pins the path a reader would
/// expect — `rstest_bdd::runner::StepOutcome` — so a re-export that moved would
/// break here rather than in a downstream crate.
#[test]
fn step_outcome_is_reachable_at_its_documented_path() {
    let _: fn(&StepOutcome) -> Option<SourceLocation> = |record| record.source().cloned();
}
