//! Standalone fixture crate kept as the negative control for the V-2 prelude
//! claim.
//!
//! Unlike its sibling `prelude_only`, this manifest names `rstest-bdd-macros`,
//! so the test that asserts the prelude fixture declares no such entry has a
//! case where the crate really is declared. Its library is deliberately
//! empty; `tests/macros.rs` holds the scenario.