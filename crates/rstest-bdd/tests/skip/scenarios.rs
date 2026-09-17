//! Skip fixtures, steps, and scenario declarations.

use rstest::fixture;
use rstest_bdd as bdd;
use rstest_bdd_macros::{given, scenario, then};
use serial_test::serial;

#[must_use]
pub struct FailOnSkippedGuard;

impl FailOnSkippedGuard {
    pub(super) fn enable() -> Self {
        bdd::config::set_fail_on_skipped(true);
        Self
    }

    pub(super) fn disable() -> Self {
        bdd::config::set_fail_on_skipped(false);
        Self
    }
}

impl Drop for FailOnSkippedGuard {
    // Clearing the override re-exposes the RSTEST_BDD_FAIL_ON_SKIPPED variable.
    // Tests using this guard must be marked #[serial(skip_reporting)] to avoid races.
    fn drop(&mut self) { bdd::config::clear_fail_on_skipped_override(); }
}

#[rstest_bdd_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn fail_on_enabled() -> FailOnSkippedGuard { FailOnSkippedGuard::enable() }

#[rstest_bdd_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn fail_on_disabled() -> FailOnSkippedGuard { FailOnSkippedGuard::disable() }

pub(super) fn assert_feature_path_suffix(actual: &str, expected_suffix: &str) {
    use std::path::Path;
    let actual_path = Path::new(actual);
    let expected = Path::new(expected_suffix);
    assert!(
        actual_path.ends_with(expected),
        "feature path should reference {expected_suffix}",
    );
}

#[given("a scenario will be skipped")]
fn skip_scenario() {
    bdd::skip!("skip requested for coverage");
}

#[given("a scenario will skip without a message")]
fn skip_scenario_without_message() {
    bdd::skip!();
}

#[given("a scenario completes successfully")]
fn scenario_completes_successfully() {}

#[then("a trailing step executes")]
fn trailing_step_should_not_run() {
    panic!("trailing step should not execute after a skip request");
}

#[scenario(path = "tests/features/skip.feature", name = "disallowed skip")]
#[serial(skip_reporting)]
#[should_panic(expected = "Scenario skipped with fail_on_skipped enabled")]
pub fn disallowed_skip(fail_on_enabled: FailOnSkippedGuard) {
    let _ = &fail_on_enabled;
    unreachable!("scenario should have failed before executing the body");
}

#[scenario(path = "tests/features/skip.feature", name = "allowed skip")]
#[serial(skip_reporting)]
pub fn allowed_skip(fail_on_enabled: FailOnSkippedGuard) {
    let _ = &fail_on_enabled;
    panic!("scenario body should not execute when skip is allowed");
}

#[scenario(
    path = "tests/features/skip.feature",
    name = "allowed skip without message"
)]
#[serial(skip_reporting)]
pub fn allowed_skip_without_message(fail_on_enabled: FailOnSkippedGuard) {
    let _ = &fail_on_enabled;
    panic!("scenario body should not execute when skip is allowed without a message");
}

#[scenario(path = "tests/features/skip.feature", name = "skip without fail flag")]
#[serial(skip_reporting)]
fn skip_without_flag(fail_on_disabled: FailOnSkippedGuard) {
    let _ = &fail_on_disabled;
    panic!("scenario body should not execute when fail_on_skipped is disabled");
}

#[scenario(
    path = "tests/features/skip.feature",
    name = "skip prevents trailing steps"
)]
#[serial(skip_reporting)]
fn skip_prevents_trailing_steps(fail_on_disabled: FailOnSkippedGuard) {
    let _ = &fail_on_disabled;
    panic!("scenario body should not execute when earlier steps skip");
}

#[scenario(
    path = "tests/features/skip_allowance/feature_tag.feature",
    name = "inherits feature tag"
)]
#[serial(skip_reporting)]
fn feature_tag_allows_skip(fail_on_enabled: FailOnSkippedGuard) {
    let _ = &fail_on_enabled;
    panic!("scenario body should not execute when feature-level tags allow skipping");
}

#[scenario(
    path = "tests/features/skip_allowance/example_tag.feature",
    name = "example tag ignored"
)]
#[serial(skip_reporting)]
#[should_panic(expected = "Scenario skipped with fail_on_skipped enabled")]
fn example_tag_does_not_allow_skip(fail_on_enabled: FailOnSkippedGuard, case: String) {
    let _ = case;
    let _ = &fail_on_enabled;
}

#[scenario(path = "tests/features/reporting.feature", name = "scenario passes")]
#[serial(skip_reporting)]
pub fn scenario_passes_without_skip() {}

#[path = "scenarios/collector.rs"]
mod collector;
#[cfg(feature = "diagnostics")]
#[path = "scenarios/json.rs"]
mod json;
#[cfg(feature = "diagnostics")]
#[path = "scenarios/junit.rs"]
mod junit;
