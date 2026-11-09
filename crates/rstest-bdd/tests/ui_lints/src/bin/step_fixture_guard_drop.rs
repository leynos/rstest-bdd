//! Clippy UI fixture validating that generated step wrappers release fixture
//! borrow guards straight after the step call, under
//! `-D clippy::significant_drop_tightening`.
//!
//! The steps cover shared-reference, mutable-reference and owned fixtures in
//! both synchronous and asynchronous step functions.

#![deny(clippy::significant_drop_tightening)]

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

/// Clippy only checks guards whose type has a significant `Drop`, such as a
/// fixture holding a lock guard or a connection. The attribute marks this
/// fixture that way so the wrapper's borrow guards are subject to the lint.
#[derive(Clone, Default)]
#[clippy::has_significant_drop]
struct Ledger {
    entries: Vec<&'static str>,
}

#[fixture]
#[rustfmt::skip]
fn ledger() -> Ledger {
    Ledger::default()
}

#[given("a shared ledger reference")]
fn shared_ledger(ledger: &Ledger) {
    assert!(ledger.entries.is_empty());
}

#[when("a mutable ledger reference records an entry")]
fn record_entry(ledger: &mut Ledger) { ledger.entries.push("sync"); }

#[when("an async mutable ledger reference records an entry")]
async fn record_entry_async(ledger: &mut Ledger) { ledger.entries.push("async"); }

#[then("the owned ledger copy holds two entries")]
fn owned_ledger(ledger: Ledger) {
    assert_eq!(ledger.entries.len(), 2);
}

#[then("the async shared ledger reference holds two entries")]
async fn shared_ledger_async(ledger: &Ledger) {
    assert_eq!(ledger.entries.len(), 2);
}

#[scenario(
    path = "features/lint_significant_drop_tightening.feature",
    name = "Steps borrow fixtures by shared reference, mutable reference and value"
)]
fn steps_release_fixture_guards(ledger: Ledger) {
    assert_eq!(ledger.entries.len(), 2);
}

fn main() {}
