//! Unit tests for `assert_step_ok!` and `assert_step_err!`.

use std::panic::{catch_unwind, AssertUnwindSafe};

use rstest::rstest;
use rstest_bdd::localization::{strip_directional_isolates, ScopedLocalization};
use rstest_bdd::{assert_step_err, assert_step_ok, panic_message};
use unic_langid::langid;

fn capture_panic_message(op: impl FnOnce()) -> String {
    match catch_unwind(AssertUnwindSafe(op)) {
        Ok(()) => panic!("operation should panic"),
        Err(payload) => strip_directional_isolates(&panic_message(payload.as_ref())),
    }
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
    let message = capture_panic_message(|| std::panic::panic_any(()));
    assert!(
        message.contains("TypeId("),
        "opaque payload hints should include a TypeId, got: {message}"
    );
}

#[test]
fn panic_message_captures_formatted_arguments() {
    let message = capture_panic_message(|| panic!("boom: {}", 42));
    assert_eq!(message, "boom: 42");
}

fn assert_step_ok_panics() {
    let res: Result<(), &str> = Err("boom");
    assert_step_ok!(res);
}

fn assert_step_err_panics() {
    let res: Result<(), &str> = Ok(());
    let _ = assert_step_err!(res);
}

#[rstest]
#[case::assert_step_ok(assert_step_ok_panics as fn(), "l'étape a renvoyé une erreur")]
#[case::assert_step_err(assert_step_err_panics as fn(), "l'étape a réussi")]
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
