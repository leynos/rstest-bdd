//! Invalidation fixture: a `#[scenario]`-bound test whose expectation is
//! captured from the Gherkin text as a step argument.
//!
//! The 10.3.3 regression tests copy this crate to
//! `target/tests/rebuild-invalidation/fixture`, compile it, and rewrite ONLY
//! the `Then` step's numeric argument in `tests/features/invalidation.feature`
//! (see the warning block at the top of that file). Because the expectation
//! is captured as a step argument from the Gherkin text, editing the `.feature`
//! file genuinely changes the compiled expectation: when the bound file is
//! re-read at macro-expansion time, `check_expectation` receives the new value
//! and the `assert_eq!` fails against the captured one.
//!
//! The `assert_eq!` failure message is what names the new expectation in the
//! test output — it is the load-bearing proof that the binary was recompiled
//! from the new text, because that string exists only in the new `.feature`.

use rstest_bdd_macros::{given, scenario, scenarios, then};
use std::sync::atomic::{AtomicU32, Ordering};

/// The value captured by the `Given` step; the `Then` step compares it
/// against the value bound in the `.feature` text.
static CAPTURED: AtomicU32 = AtomicU32::new(0);

#[given("the captured value is {value:u32}")]
fn capture_value(value: u32) {
    CAPTURED.store(value, Ordering::SeqCst);
}

#[then("the bound expectation is {value:u32}")]
fn check_expectation(value: u32) {
    assert_eq!(CAPTURED.load(Ordering::SeqCst), value);
}

#[scenario(path = "tests/features/invalidation.feature")]
fn invalidation_scenario() {}

#[given("a scenario-level tracking step")]
fn tracking_step() {}

// Directory-bound scenarios with a `tags =` filter that excludes every
// scenario in `no_match.feature` (`@excluded`) while `match.feature`
// (`@wanted`) still generates a test. The 10.3.3 dep-info contract asserts
// that the *filtered-out* file is tracked too — the file was parsed, so an
// edit to it must rebuild, even though no test is generated from it.
scenarios!(
    "tests/features/scenarios_dir",
    tags = "@wanted",
);