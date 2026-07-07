//! First binding for the bulk-migration cookbook shared step library.
//!
//! This file defines no steps of its own: it includes the shared step library
//! via `#[path]` and binds one scenario to it, demonstrating that a consuming
//! crate reuses one durable-handle step library across many scenarios without
//! copying the helper code per scenario. The fixture provenance stays visible
//! at the binding site through the module-qualified `#[from(...)]` path.

#[path = "common/bulk_migration_steps.rs"]
mod bulk_migration_steps;

use rstest::rstest;
use rstest_bdd::ScenarioState as _;
use rstest_bdd_macros::scenario;

#[scenario(
    path = "tests/features/bulk_migration/first.feature",
    name = "First scenario reuses the shared step library"
)]
fn scenario_first_reuses_shared_steps(
    #[from(bulk_migration_steps::ledger_state)] _state: bulk_migration_steps::LedgerState,
) {
}

/// Unit-test the durable-state reset the shared library relies on, using the
/// shared `ledger_state` fixture through the same module-qualified `#[from]`
/// path as the scenario binding, so no separate import is needed.
#[rstest]
fn ledger_reset_clears_accumulated_balance(
    #[from(bulk_migration_steps::ledger_state)] ledger_state: bulk_migration_steps::LedgerState,
) {
    ledger_state.balance.set(42);
    assert_eq!(ledger_state.balance.get(), Some(42));
    ledger_state.reset();
    assert!(ledger_state.balance.is_empty());
}
