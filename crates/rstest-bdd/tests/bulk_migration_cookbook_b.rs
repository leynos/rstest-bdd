//! Second binding for the bulk-migration cookbook shared step library.
//!
//! Like the first binding, this file defines no steps: it includes the same
//! shared step library via `#[path]` and binds a second scenario, proving the
//! one library serves many scenarios across many feature files.

#[path = "common/bulk_migration_steps.rs"]
mod bulk_migration_steps;

use bulk_migration_steps::{LedgerState, ledger_state};
use rstest_bdd_macros::scenario;

#[scenario(
    path = "tests/features/bulk_migration/second.feature",
    name = "Second scenario reuses the shared step library"
)]
fn scenario_second_reuses_shared_steps(#[from(ledger_state)] _state: LedgerState) {}
