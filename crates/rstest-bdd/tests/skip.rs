//! Behavioural coverage for scenario skipping semantics.

use rstest::fixture;
use rstest_bdd as bdd;
use rstest_bdd_macros::{given, scenario, then};
use serial_test::serial;

#[must_use]
struct FailOnSkippedGuard;

impl FailOnSkippedGuard {
    fn enable() -> Self {
        bdd::config::set_fail_on_skipped(true);
        Self
    }

    fn disable() -> Self {
        bdd::config::set_fail_on_skipped(false);
        Self
    }
}

impl Drop for FailOnSkippedGuard {
    // Clearing the override re-exposes the RSTEST_BDD_FAIL_ON_SKIPPED variable.
    // Tests using this guard must be marked #[serial] to avoid races.
    fn drop(&mut self) {
        bdd::config::clear_fail_on_skipped_override();
    }
}

#[fixture]
fn fail_on_enabled() -> FailOnSkippedGuard {
    FailOnSkippedGuard::enable()
}

#[fixture]
fn fail_on_disabled() -> FailOnSkippedGuard {
    FailOnSkippedGuard::disable()
}

#[given("a scenario will be skipped")]
fn skip_scenario() {
    bdd::skip!("skip requested for coverage");
}

#[then("a trailing step executes")]
fn trailing_step_should_not_run() {
    panic!("trailing step should not execute after a skip request");
}

#[scenario(path = "tests/features/skip.feature", name = "disallowed skip")]
#[serial]
#[should_panic(expected = "Scenario skipped with fail_on_skipped enabled")]
fn disallowed_skip(fail_on_enabled: FailOnSkippedGuard) {
    let _ = &fail_on_enabled;
    unreachable!("scenario should have failed before executing the body");
}

#[scenario(path = "tests/features/skip.feature", name = "allowed skip")]
#[serial]
fn allowed_skip(fail_on_enabled: FailOnSkippedGuard) {
    let _ = &fail_on_enabled;
    panic!("scenario body should not execute when skip is allowed");
}

#[scenario(path = "tests/features/skip.feature", name = "skip without fail flag")]
#[serial]
fn skip_without_flag(fail_on_disabled: FailOnSkippedGuard) {
    let _ = &fail_on_disabled;
    panic!("scenario body should not execute when fail_on_skipped is disabled");
}

#[scenario(
    path = "tests/features/skip.feature",
    name = "skip prevents trailing steps"
)]
#[serial]
fn skip_prevents_trailing_steps(fail_on_disabled: FailOnSkippedGuard) {
    let _ = &fail_on_disabled;
    panic!("scenario body should not execute when earlier steps skip");
}
