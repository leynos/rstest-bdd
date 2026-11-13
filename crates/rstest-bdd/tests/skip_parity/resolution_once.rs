//! INV-9: `fail_on_skipped` is resolved once, at scope construction.
//!
//! INV-9 states that mutating the process-global policy cannot change a run's
//! answer, whether that run has already finished or is still in flight.
//!
//! # Why this is a separate module
//!
//! The parent module holds INV-6's parity table, which supplies the policy per
//! run through [`ScenarioScope::with_skip_policy`] and therefore touches no
//! shared state. Every row here instead arms the process-global override, so it
//! needs `#[serial]` and a guard that restores the override; keeping those two
//! disciplines in different files stops a future parity row from acquiring a
//! serial dependency it does not need.
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
//! cannot reach the registry (see `runner_wire.rs`). The rows that read the
//! *ambient* policy rather than overriding it also need the process-global
//! override, so they are `#[serial]` and restore it on the way out.

use rstest_bdd::{
    StepContext,
    StepKeyword,
    config,
    runner::{ScenarioPlanBuilder, ScenarioScope, ScenarioSkip, ScenarioStatus, run_scenario},
};
use rstest_bdd_macros::given;
use serial_test::serial;

use super::{plan, run_with};

/// Restores the `fail_on_skipped` override when it drops, panicking or not.
///
/// These tests arm the process-global override and then run a scenario, so an
/// assertion that fails between the `set` and the `clear` would unwind past the
/// clear and leave the override in place for every later test in the binary.
/// `#[serial]` keeps the other tests from *running* during this one; it does not
/// undo a leak that outlives it. Binding a guard makes the restore follow the
/// value rather than the control flow, which is why the tests below no longer
/// call `clear_fail_on_skipped_override` themselves.
///
/// The seed is a parameter because the rows disagree about it: the first and
/// last begin from a tolerated policy and let a handler flip it, and the ambient
/// row begins from a forced one to read that path. Dropping the guard clears
/// rather than restores, which re-exposes the environment variable, so the seed
/// holds only for the test's own body — which is all any of them depends on.
#[must_use]
struct FailOnSkippedOverride;

impl FailOnSkippedOverride {
    /// Set the override to `seed`, returning the guard that will clear it.
    fn armed(seed: bool) -> Self {
        config::set_fail_on_skipped(seed);
        Self
    }
}

impl Drop for FailOnSkippedOverride {
    fn drop(&mut self) { config::clear_fail_on_skipped_override(); }
}

/// A step that flips the global policy *from inside a handler*, then skips.
///
/// This is INV-9's literal antecedent. The other tests here mutate the global
/// between runs, which shows that a scope reads it once at construction — but
/// not that a mutation arriving *during* a run is ignored, because nothing in
/// those runs mutates anything. Here the mutation and the skip happen in the
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

/// INV-9, forwards: flipping the global to `true` inside a step does not
/// retroactively force a skip the run began tolerating.
///
/// The run is started under the *override*, so nothing here depends on the
/// ambient environment; only the mutation inside the handler touches state, and
/// the assertion is that the run's answer was already fixed before that happened.
#[test]
#[serial]
fn a_policy_flip_inside_a_step_cannot_reach_a_run_that_already_resolved() {
    let _policy = FailOnSkippedOverride::armed(false);
    let outcome = run_with(false, false);

    // The flip is what the *next* run would see, and the test asserts that too,
    // so a pass cannot come from the mutation having failed to happen.
    config::set_fail_on_skipped(true);
    let later = run_with(false, false);

    assert!(
        !outcome.skip().is_some_and(ScenarioSkip::forced_failure),
        "the first run resolved `fail_on_skipped = false` and must keep it",
    );
    assert!(
        !later.skip().is_some_and(ScenarioSkip::forced_failure),
        concat!(
            "the second run was given its policy explicitly, so the global must not override the ",
            "per-run value either",
        ),
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
    // Seeded `true` and flipped by the closure below; the guard clears it after
    // the assertions, which is also the state a following test must not depend
    // on, since these rows are `#[serial]` but a leaked override would outlive
    // the serialization.
    let _policy = FailOnSkippedOverride::armed(true);
    let run = |fail_on_skipped: bool| {
        config::set_fail_on_skipped(fail_on_skipped);
        let mut ctx = StepContext::default();
        let scope = ScenarioScope::new(&mut ctx);
        run_scenario(&plan(false), scope)
    };

    let forced = run(true);
    let tolerated = run(false);

    assert!(
        forced.skip().is_some_and(ScenarioSkip::forced_failure),
        "with the global set, a disallowed skip must be forced",
    );
    assert!(
        !tolerated.skip().is_some_and(ScenarioSkip::forced_failure),
        "with the global clear, the same plan must not be forced",
    );
}

/// INV-9, ambiently: a flip from *inside* a step cannot change the in-flight
/// run's answer when the scope took its policy from the global.
///
/// The explicit-policy row below shows that a flip cannot reach a run whose
/// answer was overridden. It cannot show the same for the ambient path,
/// because `with_skip_policy` replaces the value `ScenarioScope::new` read and
/// the override is then the only input. Without this row a driver that
/// re-read the global only when the policy was ambient would pass every other
/// test in this module.
///
/// The scope is constructed under an armed `false`, and the handler flips the
/// global to `true` before skipping. A driver that resolved lazily at skip time
/// would therefore report a forced failure, while the value the scope stored at
/// construction says it must not.
#[test]
#[serial]
fn an_ambient_policy_flip_inside_a_step_cannot_change_its_own_run() {
    let _policy = FailOnSkippedOverride::armed(false);
    let mut ctx = StepContext::default();
    let scope = ScenarioScope::new(&mut ctx);

    let outcome = run_scenario(&flipping_plan(), scope);
    let flipped_global = config::fail_on_skipped();

    assert!(
        flipped_global,
        concat!(
            "the handler must have actually flipped the global, or this test proves nothing about ",
            "mid-run mutation of the ambient source",
        ),
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
        concat!(
            "the scope resolved `fail_on_skipped = false` from the global before the handler ran, ",
            "so a driver that re-read the global while building this record would be observable ",
            "here and only here",
        ),
    );
    assert!(
        skip.allow_skipped(),
        concat!(
            "the record's `allow_skipped` must still be the effective value computed from the ",
            "scope's resolved policy; a driver that re-read the flipped global would compute ",
            "`false || !true` and report false here",
        ),
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
/// once (D10) exists to prevent, and nothing else in this module can observe it.
///
/// `allow_skipped` is left at its default (false) and `fail_on_skipped` is
/// supplied explicitly as false, so the expected answer is "not forced" and the
/// flipped global — had it been read — would have produced `true`. The
/// direction matters: with both inputs true the row would agree under every
/// candidate implementation.
#[test]
#[serial]
fn a_policy_flip_inside_a_step_cannot_change_its_own_run() {
    let _policy = FailOnSkippedOverride::armed(false);
    let mut ctx = StepContext::default();
    let scope = ScenarioScope::new(&mut ctx).with_skip_policy(false);

    // The handler flips the global; the guard has not dropped yet, so this still
    // reads the flipped value rather than the cleared one.
    let outcome = run_scenario(&flipping_plan(), scope);
    let flipped_global = config::fail_on_skipped();

    assert!(
        flipped_global,
        concat!(
            "the handler must have actually flipped the global, or this test proves nothing about ",
            "mid-run mutation",
        ),
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
        concat!(
            "the run resolved `fail_on_skipped = false` before the handler ran, so a driver that ",
            "re-read the global while building this record would be observable here and only here",
        ),
    );
    assert!(
        skip.allow_skipped(),
        concat!(
            "the record's `allow_skipped` is the *effective* value — `plan_allows_skipping || ",
            "!fail_on_skipped` — not the plan's raw flag. The plan left it false, so this being true ",
            "is the scope's resolved `fail_on_skipped = false` showing through, and a driver that ",
            "re-read the flipped global would compute `false || !true` and report false here",
        ),
    );
    assert_eq!(
        skip.message(),
        Some("parity flipped the policy mid-run"),
        "the reason the handler gave must survive the flip",
    );
}
