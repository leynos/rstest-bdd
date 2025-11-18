//! Helpers shared by skip assertion macros.

use crate::reporting::SkippedScenario;
use crate::{panic_localized, StepExecution};

#[doc(hidden)]
pub fn __rstest_bdd_expect_skip_message_contains(
    actual: Option<&str>,
    expected: &str,
    target: &'static str,
) {
    if let Some(message) = actual {
        if message.contains(expected) {
            return;
        }
        panic_localized!(
            "assert-skip-missing-substring",
            actual = message,
            expected = expected,
        );
    } else {
        panic_localized!(
            "assert-skip-missing-message",
            target = target,
            expected = expected,
        );
    }
}

#[doc(hidden)]
pub fn __rstest_bdd_expect_skip_message_absent(actual: Option<&str>, target: &'static str) {
    if actual.is_some() {
        panic_localized!("assert-skip-unexpected-message", target = target);
    }
}

#[doc(hidden)]
pub fn __rstest_bdd_unwrap_step_skipped(exec: StepExecution) -> Option<String> {
    match exec {
        StepExecution::Skipped { message } => message,
        StepExecution::Continue { .. } => {
            panic_localized!("assert-skip-not-skipped", target = "step execution")
        }
    }
}

#[doc(hidden)]
pub fn __rstest_bdd_assert_step_skipped_message_contains<E: Into<String>>(
    exec: StepExecution,
    expected: E,
) -> Option<String> {
    let message = __rstest_bdd_unwrap_step_skipped(exec);
    let expected_str: String = expected.into();
    __rstest_bdd_expect_skip_message_contains(
        message.as_deref(),
        expected_str.as_str(),
        "step execution",
    );
    message
}

#[doc(hidden)]
#[must_use]
pub fn __rstest_bdd_assert_step_skipped_message_absent(
    exec: StepExecution,
    expect_absent: bool,
) -> Option<String> {
    let message = __rstest_bdd_unwrap_step_skipped(exec);
    if expect_absent {
        __rstest_bdd_expect_skip_message_absent(message.as_deref(), "step execution");
    }
    message
}

#[doc(hidden)]
pub fn __rstest_bdd_expect_skip_flag(
    actual: bool,
    expected: bool,
    target: &'static str,
    flag: &'static str,
) {
    if actual != expected {
        panic_localized!(
            "assert-skip-flag-mismatch",
            target = target,
            flag = flag,
            expected = expected,
            actual = actual,
        );
    }
}

#[doc(hidden)]
pub fn __rstest_bdd_assert_scenario_detail_message_contains<E: Into<String>>(
    details: &SkippedScenario,
    expected: E,
) {
    let expected_str: String = expected.into();
    __rstest_bdd_expect_skip_message_contains(
        details.message(),
        expected_str.as_str(),
        "scenario status",
    );
}

#[doc(hidden)]
pub fn __rstest_bdd_assert_scenario_detail_message_absent(
    details: &SkippedScenario,
    expect_absent: bool,
) {
    if expect_absent {
        __rstest_bdd_expect_skip_message_absent(details.message(), "scenario status");
    }
}

#[doc(hidden)]
pub fn __rstest_bdd_assert_scenario_detail_flag(
    details: &SkippedScenario,
    flag_name: &'static str,
    flag_value: bool,
    expected: bool,
) {
    let _ = details;
    __rstest_bdd_expect_skip_flag(flag_value, expected, "scenario status", flag_name);
}
