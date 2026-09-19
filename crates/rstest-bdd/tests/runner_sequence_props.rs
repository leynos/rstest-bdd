//! INV-1, INV-2, INV-3, and INV-12's property half: sequence behaviour.
//!
//! These four statements are about what a run does once a previous invocation
//! has ended it, so none can be witnessed by a single run. "No invocation past
//! the terminal index is executed" is vacuous for a plan of length one and only
//! interesting for a plan whose terminal sits in the middle. The generator
//! therefore produces *plans*, and every test here runs one through
//! `run_scenario` and inspects what came back.
//!
//! - **INV-1, termination.** The run stops at the terminal index and executes nothing past it. The
//!   evidence is the execution log the steps themselves append to, not `outcome.steps()`: the
//!   latter records what the driver *says* it did, and a driver that ran a bypassed invocation and
//!   then recorded it as `Bypassed` would satisfy every assertion drawn from it.
//! - **INV-2, completeness and ordering.** `steps()` has one entry per invocation, in plan order,
//!   each carrying its own keyword, text, and source, with post-terminal entries marked `Bypassed`.
//! - **INV-3, returned-value visibility.** A value returned by invocation `i` reaches every `j > i`
//!   and no `j <= i`. A returning step hands back a [`Probe`](sequence::steps::Probe) carrying its
//!   own index, so an observer's reading names the producer it saw rather than merely proving that
//!   something was inserted.
//! - **INV-12, no silent drops.** Every value-returning invocation records its `InsertOutcome`, and
//!   the generator reaches all three [`ValueFate`]s. `NoMatch` is the one that matters: it emits no
//!   warning anywhere, so a renamed fixture would leave a suite green while later steps read a
//!   default.
//!
//! # Why the classification is accumulated rather than asserted per case
//!
//! Every one of these is satisfied by a generator that produces nothing
//! interesting. A run that never stops early satisfies INV-1 trivially; a run
//! with no value-returning step satisfies INV-3 and INV-12 trivially. Asserting
//! only inside the cases would therefore let a generator regression turn the
//! suite *green* rather than red, which is the failure mode these invariants
//! exist to prevent. Each test folds what it saw into a [`Witnesses`] and
//! asserts *after* the run that every class the invariant names was reached.
//!
//! That is why these tests drive [`TestRunner`] directly instead of using the
//! `proptest!` macro: the non-vacuity assertion must run once, after all cases,
//! and the macro's body runs per case. `prop_assert!` still works inside the
//! closure, so a failing case reports its shrunk counter-example.
//!
//! The closures are therefore `FnMut`, not `Fn`, because folding a case in
//! mutates the accumulator. A `Fn` bound here would not compile, and a
//! `RefCell` worked around it by panicking on re-entry rather than at the
//! point of the mistake.
//!
//! # Why the case budget is pinned here
//!
//! `Config::default`'s budget is replaced by [`CASES`]. Nextest runs with
//! `slow-timeout = { period = "60s", terminate-after = 1 }`, and one case is a
//! real scenario run through the registry; pinning the budget keeps the binary
//! far inside that period even when every case generates [`MAX_STEPS`]
//! invocations and every invocation panics.
//!
//! # Why this is an integration test
//!
//! D21: the unit-test binary cannot reach the registry, so a runner test that
//! merely *executes* a step must live here. See `runner_wire.rs`.
//!
//! # INV-5 is not in this file
//!
//! INV-5, sync/async equivalence, needs `run_scenario_async`, which does not
//! exist until EP-M3. It is recorded here rather than silently omitted so that
//! a reader looking for it in the file the plan names does not conclude it was
//! dropped. INV-5's own clause lands with EP-M3.
//!
//! # How the file is laid out
//!
//! Split into three modules to stay inside the repository's 400-line cap, along
//! the seams the invariants themselves provide. [`invariants`] holds the four
//! statements' property halves, [`named_witnesses`] the hand-written per-kind
//! and per-class witnesses, and [`controls`] the negative controls and the
//! checks on the generator's own domain. The shared driver and the plan builder
//! stay here, because all three call them.

use std::cell::RefCell;

use proptest::test_runner::{TestCaseError, TestRunner};
use sequence::{Arrangement, CASES, Kind, Step, Witnesses, case};

// The support modules sit beside this file rather than under a directory named
// after it, because a Cargo integration target is a single `.rs` file: `mod
// controls;` here would be looked for as `tests/controls.rs`, not as
// `tests/runner_sequence_props/controls.rs`. `#[path]` is the repository's
// established way of colocating them; see `runner_panics.rs` and
// `runner_instrumentation.rs`.
#[path = "runner_sequence_props/controls.rs"]
mod controls;
#[path = "runner_sequence_props/invariants.rs"]
mod invariants;
#[path = "runner_sequence_props/named_witnesses.rs"]
mod named_witnesses;
#[path = "runner_sequence_props/sequence/mod.rs"]
mod sequence;

/// Drive `body` over the generator with the pinned case budget.
///
/// `TestRunner::run` requires `Fn`, and a closure that mutates a captured
/// accumulator is only `FnMut` — so the accumulator has to live behind
/// interior mutability. That is not a workaround for the folding properties
/// being unusual; it is the direct consequence of proptest's signature. It is
/// safe here because proptest drives cases one at a time, so the borrow is
/// never contended and a re-entrant borrow would be a bug in proptest rather
/// than a possibility to design around.
fn check(body: impl FnMut(Vec<Step>, Arrangement) -> Result<(), TestCaseError>) {
    let mut runner = TestRunner::new(proptest::test_runner::Config {
        cases: CASES,
        ..proptest::test_runner::Config::default()
    });
    let body = RefCell::new(body);
    let result = runner.run(&case(), |(steps, arrangement)| {
        body.borrow_mut()(steps, arrangement)
    });
    assert!(result.is_ok(), "the property failed: {result:?}");
}

/// A plan from `kinds`, each tagged with the keyword its kind is registered
/// under and a distinct line.
fn plan(kinds: &[Kind]) -> Vec<Step> {
    kinds
        .iter()
        .enumerate()
        .map(|(index, &kind)| Step {
            kind,
            keyword: kind.keyword(),
            line: u32::try_from(index).unwrap_or(0) + 1,
        })
        .collect()
}

/// Fold every case the generator produces into a fresh accumulator.
///
/// The properties that need a [`Witnesses`] differ only in what they assert
/// inside the case, and one of them asserts nothing at all — it exists solely
/// to build the accumulator that `witnesses::the_generator_reaches_each_class_often_enough`
/// reads. Sharing the fold keeps that one from being a copy of the others with
/// the interesting part deleted.
fn collect_witnesses() -> Witnesses {
    let mut witnesses = Witnesses::default();
    check(|steps, arrangement| {
        let run = sequence::run_case(&steps, arrangement);
        witnesses.record(&steps, &run);
        Ok(())
    });
    witnesses
}
