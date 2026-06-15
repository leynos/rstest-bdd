//! Diagnostic computation and publishing for LSP.
//!
//! This module computes diagnostics for consistency issues between feature
//! files and Rust step definitions, publishing them via the LSP protocol.
//! Diagnostics are triggered on file save and report:
//!
//! - **Unimplemented feature steps**: Steps in `.feature` files with no
//!   matching Rust implementation.
//! - **Unused step definitions**: Rust step definitions not matched by any
//!   feature step.
//! - **Placeholder count mismatches**: Step patterns with a different number
//!   of placeholders than the function has step arguments.
//! - **Table/docstring expectation mismatches**: Feature steps with tables or
//!   docstrings that don't match what the Rust implementation expects.
//! - **Scenario outline column mismatches**: Scenario outlines with
//!   placeholders that don't match the Examples table columns.

mod compute;
mod placeholder;
mod publish;
mod scenario_outline;
mod table_docstring;

/// Diagnostic source identifier for rstest-bdd diagnostics.
const DIAGNOSTIC_SOURCE: &str = "rstest-bdd";

/// Diagnostic code for unimplemented feature steps.
const CODE_UNIMPLEMENTED_STEP: &str = "unimplemented-step";

/// Diagnostic code for unused step definitions.
const CODE_UNUSED_STEP_DEFINITION: &str = "unused-step-definition";

/// Diagnostic code for placeholder count mismatch in step definitions.
const CODE_PLACEHOLDER_COUNT_MISMATCH: &str = "placeholder-count-mismatch";

/// Diagnostic code for step expecting a data table but feature doesn't provide one.
const CODE_TABLE_EXPECTED: &str = "table-expected";

/// Diagnostic code for feature providing a data table but step doesn't expect one.
const CODE_TABLE_NOT_EXPECTED: &str = "table-not-expected";

/// Diagnostic code for step expecting a docstring but feature doesn't provide one.
const CODE_DOCSTRING_EXPECTED: &str = "docstring-expected";

/// Diagnostic code for feature providing a docstring but step doesn't expect one.
const CODE_DOCSTRING_NOT_EXPECTED: &str = "docstring-not-expected";

/// Diagnostic code for scenario outline placeholder with no matching Examples column.
const CODE_EXAMPLE_COLUMN_MISSING: &str = "example-column-missing";

/// Diagnostic code for Examples column not referenced by any step placeholder.
const CODE_EXAMPLE_COLUMN_SURPLUS: &str = "example-column-surplus";

// Re-export public items
pub use compute::{compute_unimplemented_step_diagnostics, compute_unused_step_diagnostics};
pub use placeholder::compute_signature_mismatch_diagnostics;
pub use publish::{
    publish_all_feature_diagnostics, publish_feature_diagnostics, publish_rust_diagnostics,
};
pub use scenario_outline::compute_scenario_outline_column_diagnostics;
pub use table_docstring::compute_table_docstring_mismatch_diagnostics;

#[cfg(test)]
mod tests;
