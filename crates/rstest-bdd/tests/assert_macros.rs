//! Unit tests for `assert_step_ok!` and `assert_step_err!`.

use std::panic::{catch_unwind, AssertUnwindSafe};

use rstest::rstest;
use rstest_bdd::localization::{strip_directional_isolates, ScopedLocalization};
use rstest_bdd::reporting::{ScenarioStatus, SkippedScenario};
use rstest_bdd::{
    assert_scenario_skipped, assert_step_err, assert_step_ok, assert_step_skipped, panic_message,
    StepExecution,
};
use unic_langid::langid;

fn capture_panic_message(op: impl FnOnce()) -> String {
    match catch_unwind(AssertUnwindSafe(op)) {
        Ok(()) => panic!("operation should panic"),
        Err(payload) => strip_directional_isolates(&panic_message(payload.as_ref())),
    }
}

fn panic_with_owned_string() {
    std::panic::panic_any(String::from("owned"));
}

fn panic_with_static_str() {
    std::panic::panic_any("static str");
}

fn panic_with_i32() {
    std::panic::panic_any(42_i32);
}

fn panic_with_f64() {
    std::panic::panic_any(2.5_f64);
}

fn panic_with_bool_true() {
    std::panic::panic_any(true);
}

fn panic_with_empty_string() {
    std::panic::panic_any(String::new());
}

fn panic_with_unicode_str() {
    std::panic::panic_any("résumé");
}

/// Helper to test panic messages in French locale.
fn assert_panic_in_french(op: impl FnOnce(), expected_substring: &str) {
    let _guard = ScopedLocalization::new(&[langid!("fr")])
        .unwrap_or_else(|error| panic!("failed to scope French locale: {error}"));
    let message = capture_panic_message(op);
    assert!(
        message.contains(expected_substring),
        "expected message to contain '{expected_substring}', got: {message}"
    );
}

#[test]
fn panic_message_reports_type_hint_for_opaque_payload() {
    struct Opaque;

    let message = capture_panic_message(|| std::panic::panic_any(Opaque));
    let has_type_identifier = message.contains("TypeId(") || message.contains("TypeId {");
    assert!(
        has_type_identifier,
        "opaque payload hints should include a TypeId, got: {message}"
    );
}

#[test]
fn panic_message_captures_formatted_arguments() {
    let message = capture_panic_message(|| panic!("boom: {}", 42));
    assert_eq!(message, "boom: 42");
}

#[test]
fn panic_message_downcasts_unit_type() {
    let message = capture_panic_message(|| std::panic::panic_any(()));
    assert_eq!(message, "()");
}

#[test]
fn panic_message_downcasts_boxed_str() {
    let payload: Box<str> = "boom".into();
    let message = capture_panic_message(|| std::panic::panic_any(payload));
    assert_eq!(message, "boom");
}

#[rstest]
#[case::owned_string(panic_with_owned_string as fn(), "owned")]
#[case::static_str(panic_with_static_str as fn(), "static str")]
#[case::i32(panic_with_i32 as fn(), "42")]
#[case::f64(panic_with_f64 as fn(), "2.5")]
#[case::bool_true(panic_with_bool_true as fn(), "true")]
#[case::empty_owned(panic_with_empty_string as fn(), "")]
#[case::unicode(panic_with_unicode_str as fn(), "résumé")]
fn panic_message_downcasts_common_payloads(#[case] operation: fn(), #[case] expected: &str) {
    let message = capture_panic_message(operation);
    assert_eq!(message, expected);
}

fn assert_step_ok_panics() {
    let res: Result<(), &str> = Err("boom");
    assert_step_ok!(res);
}

fn assert_step_err_panics() {
    let res: Result<(), &str> = Ok(());
    let _ = assert_step_err!(res);
}

fn assert_step_skipped_panics() {
    let _ = assert_step_skipped!(StepExecution::Continue { value: None });
}

fn assert_scenario_skipped_panics() {
    let _ = assert_scenario_skipped!(ScenarioStatus::Passed);
}

#[rstest]
#[case::assert_step_ok(assert_step_ok_panics as fn(), "l'étape a renvoyé une erreur")]
#[case::assert_step_err(assert_step_err_panics as fn(), "l'étape a réussi")]
#[case::assert_step_skipped(assert_step_skipped_panics as fn(), "aurait dû signaler une étape ignorée")]
#[case::assert_scenario_skipped(assert_scenario_skipped_panics as fn(), "aurait dû signaler une étape ignorée")]
fn assert_macros_panic_on_err_in_french(#[case] operation: fn(), #[case] expected_substring: &str) {
    assert_panic_in_french(operation, expected_substring);
}

#[test]
fn assert_step_ok_unwraps_result() {
    let res: Result<(), &str> = Ok(());
    assert_step_ok!(res);
}

#[test]
fn assert_step_ok_returns_value() {
    let res: Result<u32, &str> = Ok(42);
    let v = assert_step_ok!(res);
    assert_eq!(v, 42);
}

#[test]
fn assert_step_ok_panics_on_err_in_english() {
    let message = capture_panic_message(|| {
        let res: Result<(), &str> = Err("boom");
        assert_step_ok!(res);
    });
    assert_eq!(message, "step returned error: boom");
}

#[test]
fn assert_step_err_unwraps_error() {
    let res: Result<(), &str> = Err("boom");
    let e = assert_step_err!(res, "boo");
    assert_eq!(e, "boom");
}

#[test]
fn assert_step_err_handles_custom_error_type() {
    #[derive(Debug, PartialEq)]
    struct CustomErr(&'static str);

    impl std::fmt::Display for CustomErr {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(self.0)
        }
    }

    let res: Result<(), CustomErr> = Err(CustomErr("boom"));
    let e = assert_step_err!(res, "boom");
    assert_eq!(e, CustomErr("boom"));
}

#[test]
fn assert_step_err_unwraps_error_without_substring() {
    let res: Result<(), &str> = Err("boom");
    let e = assert_step_err!(res);
    assert_eq!(e, "boom");
}

#[test]
fn assert_step_err_accepts_owned_string_pattern() {
    let res: Result<(), &str> = Err("boom");
    let pat = "boom".to_string();
    let e = assert_step_err!(res, pat);
    assert_eq!(e, "boom");
}

#[test]
fn assert_step_err_panics_when_substring_absent() {
    let message = capture_panic_message(|| {
        let res: Result<(), &str> = Err("boom");
        let _ = assert_step_err!(res, "absent");
    });
    assert!(message.contains("does not contain"));
}

#[test]
fn assert_step_err_panics_on_ok_in_english() {
    let message = capture_panic_message(|| {
        let res: Result<(), &str> = Ok(());
        let _ = assert_step_err!(res);
    });
    assert_eq!(message, "step succeeded unexpectedly");
}

#[test]
fn assert_step_skipped_returns_message() {
    let message = assert_step_skipped!(
        StepExecution::skipped(Some("pending dependency".into())),
        message = "pending",
    );
    assert_eq!(message, Some("pending dependency".into()));
}

#[test]
fn assert_step_skipped_accepts_none_expectation() {
    let outcome = StepExecution::skipped(None);
    let message = assert_step_skipped!(outcome, message_absent = true);
    assert!(message.is_none());
}

#[test]
fn assert_step_skipped_requires_message_when_requested() {
    let message = capture_panic_message(|| {
        let outcome = StepExecution::skipped(None);
        let _ = assert_step_skipped!(outcome, message = "pending");
    });
    assert!(message.contains("skip message"));
}

#[test]
fn assert_scenario_skipped_returns_details() {
    let status = ScenarioStatus::Skipped(SkippedScenario::new(
        Some("pending upstream".into()),
        true,
        false,
    ));
    let details = assert_scenario_skipped!(
        status,
        message = "pending",
        allow_skipped = true,
        forced_failure = false,
    );
    assert_eq!(details.message(), Some("pending upstream"));
}

#[test]
fn assert_scenario_skipped_accepts_references() {
    let status = ScenarioStatus::Skipped(SkippedScenario::new(None, false, true));
    let details = assert_scenario_skipped!(&status, forced_failure = true);
    assert!(details.forced_failure());
}

#[test]
fn assert_scenario_skipped_requires_matching_flags() {
    let message = capture_panic_message(|| {
        let status = ScenarioStatus::Skipped(SkippedScenario::new(None, true, false));
        let _ = assert_scenario_skipped!(status, allow_skipped = false);
    });
    assert!(message.contains("flag 'allow_skipped'"));
}

#[test]
fn assert_scenario_skipped_detects_unexpected_message() {
    let message = capture_panic_message(|| {
        let status =
            ScenarioStatus::Skipped(SkippedScenario::new(Some("pending".into()), true, false));
        let _ = assert_scenario_skipped!(status, message_absent = true);
    });
    assert!(message.contains("not to provide"));
}
