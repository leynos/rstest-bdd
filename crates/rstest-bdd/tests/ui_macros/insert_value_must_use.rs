//! Compile-fail fixture: discarding `insert_value` trips `#[must_use]`.
//!
//! `InsertOutcome` is `#[must_use]` precisely so a silently dropped step return
//! cannot pass unnoticed. That contract is a compile-time guarantee, so it is
//! pinned here rather than by a runtime test: `deny(unused_must_use)` promotes
//! the warning to an error, and the snapshot records the diagnostic a caller
//! actually sees.
//!
//! The scenario runner is the one place discarding is correct, and it says so
//! with an explicit `let _ = ...` binding, which this lint deliberately allows.

#![deny(unused_must_use)]

use rstest_bdd::StepContext;

fn main() {
    let mut ctx = StepContext::default();
    let value = 7_u32;
    ctx.insert("number", &value);

    // Discarding the outcome hides whether the override was recorded at all.
    ctx.insert_value(Box::new(9_u32));
}
