//! Test fixture that exercises the deprecation warning for `runtime = "tokio-current-thread"`.
//!
//! This module intentionally uses the `scenarios!` macro with the deprecated
//! `runtime = "tokio-current-thread"` compatibility alias, forcing a compile error
//! via `compile_error!` so that `trybuild` can capture the deprecation warning
//! emitted by the macro. The test verifies that the macro produces the expected
//! deprecation diagnostics recommending `harness = TokioHarness` as the canonical form.

use rstest_bdd_macros::{given, scenarios, then, when};

#[given("a precondition")]
fn a_precondition() {}

#[when("an action occurs")]
fn an_action_occurs() {}

#[then("a result is produced")]
fn a_result_is_produced() {}

scenarios!(
    "basic.feature",
    runtime = "tokio-current-thread"
);

// Force a compile error so trybuild captures the deprecation warning emitted by the macro.
compile_error!("forced failure");

fn main() {}
