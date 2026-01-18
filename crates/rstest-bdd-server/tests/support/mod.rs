//! Shared test support utilities for rstest-bdd-server integration tests.
//!
//! This module re-exports utilities from the crate's `test_support` module,
//! providing a single source of truth for test infrastructure.

// Each integration test binary compiles this support module independently,
// and each uses a different subset of these re-exports. Item-level #[expect]
// fails with "unfulfilled lint expectation" when a binary uses all imports.
// Item-level #[allow] triggers clippy::allow_attributes. File-level allow is
// the only workable solution for cross-binary shared support modules.
#![allow(unused_imports, reason = "each test binary uses a different subset")]

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
