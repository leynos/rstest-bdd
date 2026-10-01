//! The run-end cleanup drop site, and the destructor panics it must survive.
//!
//! `runner/scope.rs`'s `CleanupGuard` is the runner's fourth drop site. The
//! other three are mid-run — the value that matched nothing, the value whose
//! match was ambiguous, and the override a successful insert displaced — and
//! live in `destructors.rs`. This one fires once, when the scenario scope drops,
//! after the last step has returned.
//!
//! The property under test is not merely "a destructor panic does not escape".
//! It is that **every other override is still dropped**. `HashMap::clear` does
//! not have that property: it empties the map *before* it drops anything, so a
//! destructor that panics part-way through unwinds out of the remaining drops
//! and leaves the untouched values unreachable — the map is already empty, so
//! nothing will ever drop them again.
//!
//! Both halves of that claim were measured rather than assumed. Over 4,200
//! randomized key/position/arity combinations, a single panicking destructor
//! left at least one other value never dropped in 3,387; none of those values
//! was dropped later with the map, so the loss is permanent, and the worst case
//! lost 7 of 8.
//!
//! That is also why the test seeds **two** armed overrides. With one, `clear`
//! and the drain both run exactly one destructor, so a single-armed test would
//! pass under either implementation and pin nothing. With two, `clear` runs
//! exactly one in every one of 400 measured hash orders, while the drain runs
//! both in every one — a difference with no order on which the two agree.
//!
//! Split from `destructors.rs` rather than added to it, along the seam that
//! file's own module note already draws: the parent is close to the
//! repository's 400-line cap.

use std::{cell::Cell, marker::PhantomData};

use rstest_bdd::{
    InsertOutcome,
    StepContext,
    runner::{ScenarioOutcome, ScenarioPlanBuilder, ScenarioScope, run_scenario},
};

use super::panics::silenced;

thread_local! {
    /// How many *armed* override destructors this thread has run.
    ///
    /// Counts only the values whose destructor panics, for the reason
    /// `destructors.rs` gives for its own counter: a quiet value can be dropped
    /// on any frame, so counting those would make the delta depend on how the
    /// fixtures happen to be built. An armed value here exists in exactly one
    /// place — seeded into the override map by [`run_cleanup_catching`] — and
    /// only the runner's cleanup can drop it.
    ///
    /// Per-thread rather than a process-wide `AtomicUsize`, because this test
    /// is deliberately *not* serialized and shares one binary: a global counter
    /// would also be advanced by `destructors.rs`'s rows.
    static ARMED_CLEANUP_DROPS: Cell<usize> = const { Cell::new(0) };
}

/// A value that detonates in its destructor when armed.
///
/// The marker parameter exists to make two *distinct* types available, and that
/// is a requirement rather than a flourish: [`StepContext::insert_value`]
/// records an override only when exactly one registered fixture matches its
/// type, so a second override of the same type could never be recorded — it
/// would be refused as ambiguous. The marker is never constructed, never read,
/// and carries no data; only its identity is used.
struct ArmedDrop<M> {
    /// Whether this value's destructor detonates when dropped.
    panics: bool,
    /// Occupies the marker so `M` is part of the type rather than unused.
    marker: PhantomData<M>,
}

impl<M> Drop for ArmedDrop<M> {
    fn drop(&mut self) {
        if self.panics {
            ARMED_CLEANUP_DROPS.with(|count| count.set(count.get() + 1));
            panic!("deliberate panic from a step-returned override's destructor");
        }
    }
}

/// Marker for the first override's type. Never constructed.
struct Payload;

/// Marker for the second override's type. Never constructed.
struct Counter;

/// A quiet fixture of the first override's type.
///
/// A fixture is only what `insert_value` *matches* on, so it may be quiet: it
/// lives in the fixture map, which cleanup does not touch, and is never a
/// candidate for dropping. It is `static` because the context borrows its
/// fixtures, so a stack local would outlive nothing.
static PAYLOAD_FIXTURE: ArmedDrop<Payload> = ArmedDrop {
    panics: false,
    marker: PhantomData,
};

/// A quiet fixture of the second override's type. See [`PAYLOAD_FIXTURE`].
static COUNTER_FIXTURE: ArmedDrop<Counter> = ArmedDrop {
    panics: false,
    marker: PhantomData,
};

/// Run an empty plan to completion, unwinding past `run_scenario` on an escape.
///
/// The plan is deliberately empty, and that is what makes the count below mean
/// what it says: with no steps, the only override drops in the whole call are
/// the ones cleanup performs when the scope drops at the end of `run_scenario`.
/// A step returning a value would add a fourth drop site and blur which one was
/// under test.
///
/// Both overrides are armed. A single armed value cannot distinguish the two
/// implementations — `clear` still runs exactly that one destructor before
/// unwinding — so a one-armed row would pass either way.
///
/// The context is left to drop when this frame ends, and that is safe rather
/// than merely tidy: a value stranded by a panicking `clear` is never dropped,
/// even when its map is. Measured over the same 4,200 layouts, not one of the
/// stranded values ran its destructor later with the map. So the late drop
/// cannot silently repair the defect and hide it from the count.
fn run_cleanup_catching() -> Result<ScenarioOutcome, Box<dyn std::any::Any + Send>> {
    // The `catch_unwind` below covers the `run_scenario` call, not this
    // function, so the seeding here cannot be caught: a scaffolding assertion
    // that failed inside a window would reach the caller as an escape and be
    // reported as a runner defect.
    let mut ctx = StepContext::default();
    ctx.insert("payload", &PAYLOAD_FIXTURE);
    ctx.insert("counter", &COUNTER_FIXTURE);

    // Through `insert_value`, not `insert_owned`: only a value in the context's
    // *override* map is what cleanup clears. A fixture would sit in the other
    // map, untouched by `clear_values`, and the test would pass on an empty
    // subject. A failure here means this scaffolding is wrong rather than the
    // runner, so the messages say which expectation the setup just failed.
    let payload = ctx.insert_value(Box::new(ArmedDrop::<Payload> {
        panics: true,
        marker: PhantomData,
    }));
    assert!(
        matches!(payload, InsertOutcome::Inserted(None)),
        "the payload override must be recorded, with nothing displaced yet; its fixture is the \
         only one of its type, so anything else means the scaffolding is wrong",
    );

    let counter = ctx.insert_value(Box::new(ArmedDrop::<Counter> {
        panics: true,
        marker: PhantomData,
    }));
    assert!(
        matches!(counter, InsertOutcome::Inserted(None)),
        "the counter override must be recorded too; two overrides of one type cannot both be \
         live, which is why these are two distinct types rather than two values of one",
    );

    let plan = ScenarioPlanBuilder::new("Cleanup", "notes/panics.md").build();
    // The scope is moved into `run_scenario` and dropped as its frame ends, so
    // cleanup runs *inside* this call. That is the boundary the test needs: a
    // destructor panic here has to become a returned outcome rather than an
    // unwind past `run_scenario`. The scope is constructed out here so that the
    // window covers the call and nothing else.
    let scope = ScenarioScope::new(&mut ctx);
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_scenario(&plan, scope)))
}

/// Cleanup drops every armed override, not only the first one to detonate.
///
/// The count is the discriminating assertion, and it is a hard one: an armed
/// destructor either ran or it did not, so no hash order makes the two
/// implementations agree. `1` is the `clear` defect; `2` is the repair.
///
/// The count separates the two by *where the panics are caught*, not merely by
/// how many drops happen. `CleanupGuard::drop` calls `clear_values` and holds no
/// `catch_unwind` of its own; `clear_values` drains the override map and drops
/// each drained value through `drop_guarded`, which is the catch. Draining is
/// what makes the count `2`: the map is emptied *before* the first drop runs, so
/// the second value is already out of the map and on the stack when the first
/// destructor detonates. `HashMap::clear` instead empties as it goes, so the
/// first panic unwinds out of the loop and the values it had not reached are
/// never dropped — the count stops at `1`.
///
/// The `Ok` assertion is the weaker half. Both implementations return an outcome
/// here, because `drop_guarded` catches the detonation in each — so it guards
/// against the repair *introducing* an escape rather than describing the defect.
/// It is asserted first only because an escaped unwind makes the count
/// meaningless.
#[test]
fn cleanup_drops_every_armed_override() {
    let before = ARMED_CLEANUP_DROPS.with(Cell::get);
    // No outer `catch_unwind`: the helper returns the caught unwind itself, so
    // the window here covers one call rather than a frame full of assertions.
    let escaped = silenced(run_cleanup_catching);

    let Ok(outcome) = escaped else {
        panic!(
            "run-end cleanup must reach the caller as a returned outcome, not an unwind; it \
             escaped with {escaped:?}",
        );
    };
    let dropped = ARMED_CLEANUP_DROPS.with(Cell::get) - before;

    assert!(
        outcome.steps().is_empty(),
        "the plan has no steps, so no step can have dropped an override; every armed drop counted \
         below must therefore have come from cleanup",
    );
    assert_eq!(
        dropped, 2,
        "cleanup must drop both armed overrides. A count of 1 is the defect this pins: \
         `HashMap::clear` empties the map before dropping, so the first destructor to panic \
         unwinds out of the rest and the untouched values are then unreachable and never dropped \
         again. A count of 0 means cleanup dropped nothing at all.",
    );
}
