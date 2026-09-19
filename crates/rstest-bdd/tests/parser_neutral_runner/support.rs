//! The bench's state, its registered steps, and the accessors its `Then` steps
//! read through — the vocabulary the parent's scenarios share.
//!
//! Split out of the parent to keep both files inside the repository's 400-line
//! cap. What stays in the parent is the scenarios' own prose: the module
//! documentation, the `Given`/`When`/`Then` step bodies, and the three
//! `#[scenario]` functions. What moves here is the `Bench` the steps read and
//! write, the four registered steps, and the two accessors — because that is
//! the part a reader has to hold in mind while reading any single step, and it
//! is the part the parent's documentation describes.
//!
//! The module is `support` and not `bench` because the fixture it exports is
//! itself named `bench`, and a module and an item of the same name cannot both
//! be in scope in the parent (`E0255`).
//!
//! # Why the steps and the accessors live together
//!
//! The two accessors are not general helpers: each one asserts the same
//! precondition the `When` step established, and each names a different
//! outcome field. A `Then` step that read `bench.outcome` directly would
//! pattern-match a `None` that means "the `When` step's `catch_unwind` caught a
//! panic" and report it as "the `When` step never ran" — which is the one
//! diagnosis in this suite that has to be exactly right, since INV-17 is the
//! invariant under test. Keeping them beside the steps that fill those fields
//! is what makes the pairing visible.

use std::cell::RefCell;

use rstest::fixture;
use rstest_bdd::{
    StepKeyword,
    runner::{ScenarioOutcome, ScenarioPlan},
};
use rstest_bdd_macros::{given, then, when};

/// The plan under construction, and the outcome once it has been run.
#[derive(Default)]
pub(crate) struct Bench {
    /// Built by the `Given` steps, taken by the `When` step.
    pub(crate) plan: Option<ScenarioPlan>,
    /// Filled by the `When` step, read by the `Then` steps.
    pub(crate) outcome: Option<ScenarioOutcome>,
    /// Filled by the equivalence scenario's `When`, read by its `Then`.
    ///
    /// The two runners' results are kept side by side rather than compared
    /// inside the step, so a failure reports both outcomes through the
    /// assertion's own diff instead of a hand-built message.
    pub(crate) async_outcome: Option<ScenarioOutcome>,
    /// Set if running the plan unwound, which is the one thing INV-17 forbids.
    pub(crate) unwound: bool,
}

#[rstest_bdd_test_macros::allow_fixture_expansion_lints]
#[fixture]
pub(crate) fn bench() -> RefCell<Bench> { RefCell::new(Bench::default()) }

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
pub(crate) fn step_text(role: &str) -> (&'static str, StepKeyword) {
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

/// The asynchronous outcome, or a report of what went wrong.
pub(crate) fn async_outcome(bench: &RefCell<Bench>) -> ScenarioOutcome {
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
pub(crate) fn outcome(bench: &RefCell<Bench>) -> ScenarioOutcome {
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
