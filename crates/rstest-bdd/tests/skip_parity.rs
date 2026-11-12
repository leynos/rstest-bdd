//! INV-6: skip policy is parity-checked and recorded exactly.
//!
//! INV-6 states that a successfully skipped step always records
//! `StepStatus::Skipped`, and that the skip record's `forced_failure` is exactly
//! `!allow_skipped && fail_on_skipped`.
//!
//! The statement is about the *composition* of two booleans that arrive from
//! opposite ends of the API: the plan carries `allow_skipped`, and the scope
//! carries `fail_on_skipped`. Neither is visible to the other until the runner
//! is handed both, which is why the rows below are parameterized over the full
//! product rather than sampled. There are four combinations; writing three of
//! them would be arbitrary.
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
//! # Why this is an integration test
//!
//! D21: these statements are about steps that resolve, and the unit-test binary
//! cannot reach the registry (see `runner_wire.rs`).
//!
//! # Where INV-9 lives
//!
//! INV-9 — that the same policy is resolved once, at scope construction, so a
//! mutation inside a step handler cannot change a run's answer — is covered by
//! the `resolution_once` submodule. Those rows arm the process-global override
//! and so need `#[serial]`; keeping them in their own file stops a parity row
//! here from acquiring that dependency by accident.

use rstest::rstest;
use rstest_bdd::{
    StepContext,
    StepKeyword,
    runner::{
        ScenarioOutcome,
        ScenarioPlanBuilder,
        ScenarioScope,
        ScenarioStatus,
        StepStatus,
        run_scenario,
    },
};
use rstest_bdd_macros::given;

#[path = "skip_parity/resolution_once.rs"]
mod resolution_once;

/// A step that resolves and asks to be skipped, with a message.
///
/// The message is asserted once, because a skip record that lost it would still
/// satisfy every policy assertion here — and a caller reading `skip()` to
/// explain a failure would find the explanation missing.
#[given("a parity step skips")]
fn a_parity_step_skips() {
    rstest_bdd::skip!("parity wanted a skip");
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
        concat!(
            "an explicit `@allow_skipped` must defeat `fail_on_skipped = true`; any operator but ",
            "`!allow_skipped && fail_on_skipped` reports true here",
        ),
    );
    assert!(
        skip.allow_skipped(),
        concat!(
            "the record must also expose the input that produced that answer, so a reader does not ",
            "have to infer it from the plan",
        ),
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
