//! Shared test support utilities for rstest-bdd-server integration tests.
//!
//! This module re-exports utilities from the crate's `test_support` module,
//! providing a single source of truth for test infrastructure.

// Allow unused imports since different test binaries use different subsets.
#![allow(
    unused_imports,
    dead_code,
    reason = "each test binary uses a different subset of helpers"
)]

// Re-export test support utilities from the crate.
// Note: Integration tests cannot directly access #[cfg(test)] modules,
// so we re-export via the crate's public test interface.
pub use rstest_bdd_server::test_support::{DiagnosticCheckType, ScenarioBuilder, TestScenario};

mod diagnostics_helpers;
pub use diagnostics_helpers::{
    assert_feature_has_diagnostic, assert_feature_has_no_diagnostics, assert_rust_has_diagnostic,
    assert_rust_has_no_diagnostics, assert_single_diagnostic_contains, compute_feature_diagnostics,
    compute_placeholder_diagnostics, compute_rust_diagnostics, compute_table_docstring_diagnostics,
};
