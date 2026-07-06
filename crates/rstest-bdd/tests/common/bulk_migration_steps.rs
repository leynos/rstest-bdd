//! Shared durable-state step library for the bulk-migration cookbook.
//!
//! One copy of the scenario-state scaffolding and the `Given`/`When`/`Then`
//! step definitions, reused by every `bulk_migration_cookbook_*` binding via
//! `#[path]` inclusion. Steps register once per including binary through the
//! `inventory` crate, so each binary resolves them from its own registry.
//!
//! This is the harness-agnostic analogue of the GPUI durable-handle library
//! documented in the user guide: it keeps durable scenario state in a regular
//! `rstest` fixture (`ledger_state`) backed by `Slot<T>`, which is the clean
//! shape recommended when steps do not also need mutable harness context.

use rstest::fixture;
use rstest_bdd::ScenarioState as _;
use rstest_bdd::Slot;
use rstest_bdd_macros::{ScenarioState, given, then, when};

/// Durable scenario state shared across steps within one scenario.
#[derive(Default, ScenarioState)]
pub struct LedgerState {
    /// Running balance accumulated by posted entries.
    pub balance: Slot<i32>,
}

/// Fixture providing a fresh [`LedgerState`] for each scenario.
///
/// Deriving `ScenarioState` gives `LedgerState` a `reset()` that clears every
/// slot; `rstest` constructs a new instance per scenario, so no `Drop` guard is
/// needed for the fixture-based shape.
#[fixture]
pub fn ledger_state() -> LedgerState {
    LedgerState::default()
}

/// Reset a fresh ledger so a scenario starts from a known-empty balance.
#[given("a fresh ledger")]
pub fn a_fresh_ledger(ledger_state: &LedgerState) {
    ledger_state.reset();
    assert!(ledger_state.balance.is_empty());
}

/// Post an entry, accumulating it into the durable balance slot.
///
/// Accumulation (rather than overwrite) is what a multi-posting scenario
/// exercises: posting 10 then 5 must leave 15, so a `set(amount)` regression
/// would fail the balance assertion.
#[when("an entry of {amount:i32} is posted")]
pub fn an_entry_is_posted(ledger_state: &LedgerState, amount: i32) {
    let running = ledger_state.balance.get().unwrap_or_default();
    ledger_state.balance.set(running + amount);
}

/// Reset the running total mid-scenario, clearing the durable balance slot.
///
/// A scenario that resets after posting and then asserts only the post-reset
/// total makes a no-op reset observable: without the clear, the earlier posting
/// would still be counted.
#[when("the running total is reset")]
pub fn the_running_total_is_reset(ledger_state: &LedgerState) {
    ledger_state.reset();
}

/// Assert the durable balance matches the expected total.
#[then("the balance is {expected:i32}")]
pub fn the_balance_is(ledger_state: &LedgerState, expected: i32) {
    assert_eq!(ledger_state.balance.get(), Some(expected));
}
