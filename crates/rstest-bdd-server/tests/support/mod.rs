//! Shared test support utilities for rstest-bdd-server integration tests.
//!
//! This module re-exports utilities from the crate's `test_support` module,
//! providing a single source of truth for test infrastructure.

// Re-export test support utilities from the crate.
// Note: Integration tests cannot directly access #[cfg(test)] modules,
// so we re-export via the crate's public test interface.
pub use rstest_bdd_server::test_support::{DiagnosticCheckType, ScenarioBuilder, TestScenario};
