//! Compile-pass fixture mirroring the bulk-migration cookbook shape.
//!
//! Mirrors the cookbook's structural Rust snippet: a shared durable-state step
//! library lives in one module, and a scenario binding reuses it by bringing
//! the fixture into scope with `use` and binding it with `#[from]`. Kept
//! self-contained (an inline `shared` module) so trybuild can compile it
//! without staging the real `tests/common` module. The runtime reference lives
//! at `crates/rstest-bdd/tests/bulk_migration_cookbook_a.rs` and `_b.rs`.

mod shared {
    use rstest::fixture;
    use rstest_bdd::ScenarioState as _;
    use rstest_bdd::Slot;
    use rstest_bdd_macros::{ScenarioState, given, then, when};

    /// Durable scenario state shared across the cookbook steps.
    #[derive(Default, ScenarioState)]
    pub struct LedgerState {
        /// Running balance accumulated by posted entries.
        pub balance: Slot<i32>,
    }

    /// Fixture providing a fresh [`LedgerState`] per scenario.
    #[fixture]
    pub fn ledger_state() -> LedgerState {
        LedgerState::default()
    }

    /// Reset a fresh ledger so a scenario starts from an empty balance.
    #[given("a fresh ledger")]
    pub fn a_fresh_ledger(ledger_state: &LedgerState) {
        ledger_state.reset();
    }

    /// Post an entry, accumulating it into the durable balance slot.
    #[when("an entry of {amount:i32} is posted")]
    pub fn an_entry_is_posted(ledger_state: &LedgerState, amount: i32) {
        let running = ledger_state.balance.get().unwrap_or_default();
        ledger_state.balance.set(running + amount);
    }

    /// Assert the durable balance matches the expected total.
    #[then("the balance is {expected:i32}")]
    pub fn the_balance_is(ledger_state: &LedgerState, expected: i32) {
        assert_eq!(ledger_state.balance.get(), Some(expected));
    }
}

use rstest_bdd_macros::scenario;

/// Compile-checked binding that reuses the shared step library.
#[scenario(
    path = "bulk_migration_cookbook.feature",
    name = "Shared step library compiles",
)]
fn bulk_migration_cookbook_example(
    #[from(shared::ledger_state)] _state: shared::LedgerState,
) {
}

/// Compile-time guard that fails fast if the feature path changes.
const _: &str = include_str!("bulk_migration_cookbook.feature");

/// Binary entry point required by the trybuild compile-pass fixture.
fn main() {}
