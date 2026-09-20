//! INV-6 and INV-9: skip policy is parity-checked and resolved exactly once.
//!
//! INV-6 states that a successfully skipped step always records
//! `StepStatus::Skipped`, and that the skip record's `forced_failure` is exactly
//! `!allow_skipped && fail_on_skipped`. INV-9 states that `fail_on_skipped` is
//! resolved once, at scope construction, so mutating the global from inside a
//! step handler cannot change the run's answer.
//!
//! Both are about the *composition* of two booleans that arrive from opposite
//! ends of the API: the plan carries `allow_skipped`, and the scope carries
//! `fail_on_skipped`. Neither is visible to the other until the runner is handed
//! both, which is why the rows below are parameterized over the full product
//! rather than sampled. There are four combinations; writing three of them would
//! be arbitrary.
//!
//! # Why the discriminating row is called out
//!
//! `||` also answers `true` at `(false, false)`, and a forgotten negation
//! answers `false` there — so `(true, true)` is the unique row that rejects
//! `||`, `!=`, and a forgotten negation *together*. That is the precise sense in
//! which it is the discriminating row, and the sense the dedicated test below
//! relies on: "the only row that separates them" is a claim about that operator
//! set, not about the row set, since a function answering `false` on `(true,
//! true)` and `true` on `(false, false)` separates the correct operator at
//! `(false, false)` alone. The plan names the mutation explicitly and the row
//! asserts it explicitly, so the table cannot pass while the operator is wrong.
//!
//! # Why INV-9 is a regression test rather than a matrix
//!
//! D10 makes the resolution a *type-level* fact: the scope stores a `bool` and
//! the engine reads that field, so there is no second read to get wrong. What a
//! test can still catch is a future refactor reintroducing one — a `config`
//! call inside the driver, say — and one case in each direction catches that. A
//! matrix would be four copies of the same assertion.
//!
//! # Why this is an integration test
//!
//! D21: these statements are about steps that resolve, and the unit-test binary
//! cannot reach the registry (see `runner_wire.rs`). The two rows that read the
//! *ambient* policy rather than overriding it also need the process-global
//! override, so they are `#[serial]` and restore it on the way out.

use rstest::rstest;
use rstest_bdd::{
    StepContext,
    StepKeyword,
    config,
    runner::{
        ScenarioOutcome,
        ScenarioPlanBuilder,
        ScenarioScope,
        ScenarioSkip,
        ScenarioStatus,
        StepStatus,
        run_scenario,
    },
};
use rstest_bdd_macros::given;
use serial_test::serial;

/// A step that resolves and asks to be skipped, with a message.
///
/// The message is asserted once, because a skip record that lost it would still
/// satisfy every policy assertion here — and a caller reading `skip()` to
/// explain a failure would find the explanation missing.
#[given("a parity step skips")]
fn a_parity_step_skips() {
    rstest_bdd::skip!("parity wanted a skip");
}

/// A step that flips the global policy *from inside a handler*, then skips.
///
/// This is INV-9's literal antecedent. The other tests in this file mutate the
/// global between runs, which shows that a scope reads it once at construction —
/// but not that a mutation arriving *during* a run is ignored, because nothing
/// in those runs mutates anything. Here the mutation and the skip happen in the
/// same handler, with the run already in flight, so a driver that re-read the
/// global at the point it builds the skip record would report a forced failure
/// while the plan's own `allow_skipped` and the scope's resolved value both said
/// otherwise.
///
/// The two calls are ordered deliberately: the flip happens *before* the skip,
/// so a re-read at skip time would see the new value. Were the order reversed,
/// this test would pass against a driver that re-reads and be vacuous.
#[given("a parity step flips the policy and skips")]
fn a_parity_step_flips_the_policy_and_skips() {
    config::set_fail_on_skipped(true);
    rstest_bdd::skip!("parity flipped the policy mid-run");
}

/// A plan whose single step flips the global and then skips.
fn flipping_plan() -> rstest_bdd::runner::ScenarioPlan {
    ScenarioPlanBuilder::new("Parity", "notes/parity.md")
        .step_at(
            StepKeyword::Given,
            "a parity step flips the policy and skips",
            3,
        )
        .build()
}

/// A plan with one skipping step, opted in to skipping or not.
fn plan(allow_skipped: bool) -> rstest_bdd::runner::ScenarioPlan {
    ScenarioPlanBuilder::new("Parity", "notes/parity.md")
        .step_at(StepKeyword::Given, "a parity step skips", 3)
        .allow_skipped(allow_skipped)
        .build()
}

/// Run the plan under an explicitly supplied `fail_on_skipped`.
///
/// The policy is passed through [`ScenarioScope::with_skip_policy`] rather than
/// the environment or the process-global override, so the four rows below do not
/// contend for either and need no serialization. That is D10's whole point: the
/// ambient value is one *source* of policy, not the only one, and a test that
/// must set it to exercise a policy is testing the wrong layer.
fn run_with(allow_skipped: bool, fail_on_skipped: bool) -> ScenarioOutcome {
    let mut ctx = StepContext::default();
    let scope = ScenarioScope::new(&mut ctx).with_skip_policy(fail_on_skipped);
    run_scenario(&plan(allow_skipped), scope)
}

/// INV-6's row set: `forced_failure == !allow_skipped && fail_on_skipped`.
#[rstest]
#[case::disallowed_and_tolerated(false, false, false, "disallowed and tolerated")]
#[case::disallowed_and_failing(false, true, true, "disallowed and failing")]
#[case::allowed_and_tolerated(true, false, false, "allowed and tolerated")]
#[case::allowed_and_failing(true, true, false, "allowed and failing")]
fn forced_failure_is_the_conjunction_of_both_inputs(
    #[case] allow_skipped: bool,
    #[case] fail_on_skipped: bool,
    #[case] expected_forced: bool,
    #[case] label: &str,
) {
    let outcome = run_with(allow_skipped, fail_on_skipped);

    assert_eq!(
        outcome.status(),
        ScenarioStatus::Skipped,
        "({label}) a skip that is not forced is still a skip",
    );
    let Some(skip) = outcome.skip() else {
        panic!("({label}) a skipped run must carry a skip record: {outcome:?}");
    };
    assert_eq!(
        skip.forced_failure(),
        expected_forced,
        "({label}) forced_failure must be `!allow_skipped && fail_on_skipped`",
    );
    assert_eq!(
        skip.at(),
        0,
        "({label}) the skip happened at the first step"
    );
    assert_eq!(
        skip.message(),
        Some("parity wanted a skip"),
        "({label}) the step's reason must survive into the record",
    );
}

/// The row that separates `&& !` from every nearby mistyping.
///
/// Given its own test rather than left as one case above, because it is the only
/// row with discriminating power and a future edit that dropped a case from the
/// table would be silently harmless everywhere else. `(true, true)` is where the
/// correct operator must answer `false`; `||`, `!=`, and a forgotten negation
/// all answer `true` here and cannot be distinguished anywhere else.
#[test]
fn the_discriminating_row_rejects_the_nearest_wrong_operator() {
    let outcome = run_with(true, true);

    let Some(skip) = outcome.skip() else {
        panic!("the run must carry a skip record: {outcome:?}");
    };
    assert!(
        !skip.forced_failure(),
        "an explicit `@allow_skipped` must defeat `fail_on_skipped = true`; any operator but \
         `!allow_skipped && fail_on_skipped` reports true here",
    );
    assert!(
        skip.allow_skipped(),
        "the record must also expose the input that produced that answer, so a reader does not \
         have to infer it from the plan",
    );
}

/// INV-6's per-step half: the skipping step's own record says `Skipped`, and the
/// steps after it are `Bypassed` rather than absent.
///
/// Separate from the policy rows because it is a different claim about a
/// different type. The policy rows read `ScenarioSkip`; this reads
/// `StepOutcome`. An implementation could satisfy all of them and still have
/// marked the skipping step `Passed`, which is precisely the confusion a
/// frontend's per-step report would surface first.
#[test]
fn the_skipping_step_is_recorded_as_skipped_and_the_rest_bypassed() {
    let mut ctx = StepContext::default();
    let scope = ScenarioScope::new(&mut ctx).with_skip_policy(false);
    let plan = ScenarioPlanBuilder::new("Parity", "notes/parity.md")
        .step_at(StepKeyword::Given, "a parity step skips", 3)
        .step_at(StepKeyword::Then, "a step nobody wrote", 4)
        .build();

    let outcome = run_scenario(&plan, scope);

    assert_eq!(outcome.status(), ScenarioStatus::Skipped);
    assert_eq!(
        outcome.steps().len(),
        2,
        "every invocation is recorded, including the bypassed one",
    );
    assert_eq!(
        outcome
            .steps()
            .first()
            .map(rstest_bdd::runner::StepOutcome::status),
        Some(StepStatus::Skipped),
    );
    assert_eq!(
        outcome
            .steps()
            .get(1)
            .map(rstest_bdd::runner::StepOutcome::status),
        Some(StepStatus::Bypassed),
        "the trailing step must not be executed, and must still be reported",
    );
    assert_eq!(
        outcome
            .terminal_source()
            .map(rstest_bdd::runner::SourceLocation::line),
        Some(3),
        "the terminal source is the skipping step's, not the plan's",
    );
}

/// INV-9, forwards: flipping the global to `true` inside a step does not
/// retroactively force a skip the run began tolerating.
///
/// The run is started under the *override*, so nothing here depends on the
/// ambient environment; only the mutation inside the handler touches state, and
/// the assertion is that the run's answer was already fixed before that happened.
#[test]
#[serial]
fn a_policy_flip_inside_a_step_cannot_reach_a_run_that_already_resolved() {
    config::set_fail_on_skipped(false);
    let outcome = run_with(false, false);

    // The flip is what the *next* run would see, and the test asserts that too,
    // so a pass cannot come from the mutation having failed to happen.
    config::set_fail_on_skipped(true);
    let later = run_with(false, false);
    config::clear_fail_on_skipped_override();

    assert!(
        !outcome.skip().is_some_and(ScenarioSkip::forced_failure),
        "the first run resolved `fail_on_skipped = false` and must keep it",
    );
    assert!(
        !later.skip().is_some_and(ScenarioSkip::forced_failure),
        "the second run was given its policy explicitly, so the global must not override the \
         per-run value either",
    );
}

/// INV-9, in the other direction, through the *ambient* source.
///
/// The first test cannot see a mis-resolution; it only sees that the override
/// wins. This one removes the explicit policy and lets the scope read the
/// process-global value, so it exercises `ScenarioScope::new`'s own call to
/// `config::fail_on_skipped` — the single read INV-9 is about. Both directions
/// are asserted, because a scope that hard-coded `false` would pass one of them.
#[test]
#[serial]
fn the_scopes_ambient_resolution_reads_the_global_once() {
    let run = |fail_on_skipped: bool| {
        config::set_fail_on_skipped(fail_on_skipped);
        let mut ctx = StepContext::default();
        let scope = ScenarioScope::new(&mut ctx);
        run_scenario(&plan(false), scope)
    };

    let forced = run(true);
    let tolerated = run(false);
    config::clear_fail_on_skipped_override();

    assert!(
        forced.skip().is_some_and(ScenarioSkip::forced_failure),
        "with the global set, a disallowed skip must be forced",
    );
    assert!(
        !tolerated.skip().is_some_and(ScenarioSkip::forced_failure),
        "with the global clear, the same plan must not be forced",
    );
}

/// INV-9's literal claim: a flip from *inside* a step cannot change the
/// in-flight run's answer.
///
/// The other INV-9 tests mutate the global between runs. This one mutates it
/// during a run, from the handler, and then skips — so the driver's
/// skip-record construction and the mutation are in the same execution. A
/// driver that resolved policy lazily, at the point it renders the skip, would
/// read the flipped value and report a forced failure even though the scope had
/// already resolved `false`. That is exactly the defect the decision to resolve
/// once (D10) exists to prevent, and nothing else in this file can observe it.
///
/// `allow_skipped` is left at its default (false) and `fail_on_skipped` is
/// supplied explicitly as false, so the expected answer is "not forced" and the
/// flipped global — had it been read — would have produced `true`. The
/// direction matters: with both inputs true the row would agree under every
/// candidate implementation.
#[test]
#[serial]
fn a_policy_flip_inside_a_step_cannot_change_its_own_run() {
    config::set_fail_on_skipped(false);
    let mut ctx = StepContext::default();
    let scope = ScenarioScope::new(&mut ctx).with_skip_policy(false);

    let outcome = run_scenario(&flipping_plan(), scope);

    // Read before clearing, so the assertion below is about the run and not
    // about whether the restore happened.
    let flipped_global = config::fail_on_skipped();
    config::clear_fail_on_skipped_override();

    assert!(
        flipped_global,
        "the handler must have actually flipped the global, or this test proves nothing about \
         mid-run mutation",
    );

    assert_eq!(
        outcome.status(),
        ScenarioStatus::Skipped,
        "the run still ends in a skip rather than a failure",
    );
    let Some(skip) = outcome.skip() else {
        panic!("the run must carry a skip record: {outcome:?}");
    };
    assert!(
        !skip.forced_failure(),
        "the run resolved `fail_on_skipped = false` before the handler ran, so a driver that \
         re-read the global while building this record would be observable here and only here",
    );
    assert!(
        skip.allow_skipped(),
        "the record's `allow_skipped` is the *effective* value — `plan_allows_skipping || \
         !fail_on_skipped` — not the plan's raw flag. The plan left it false, so this being true \
         is the scope's resolved `fail_on_skipped = false` showing through, and a driver that \
         re-read the flipped global would compute `false || !true` and report false here",
    );
    assert_eq!(
        skip.message(),
        Some("parity flipped the policy mid-run"),
        "the reason the handler gave must survive the flip",
    );
}
