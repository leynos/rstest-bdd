//! Unit tests for `assert_step_ok!` and `assert_step_err!`.

use std::panic::{AssertUnwindSafe, catch_unwind};

use rstest_bdd::localisation::{ScopedLocalisation, strip_directional_isolates};
use rstest_bdd::{assert_step_err, assert_step_ok, panic_message};
use unic_langid::langid;

fn capture_panic_message(op: impl FnOnce()) -> String {
    match catch_unwind(AssertUnwindSafe(op)) {
        Ok(()) => panic!("operation should panic"),
        Err(payload) => strip_directional_isolates(&panic_message(payload.as_ref())),
    }
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
fn assert_step_ok_panics_on_err_in_french() {
    let guard = ScopedLocalisation::new(&[langid!("fr")])
        .unwrap_or_else(|error| panic!("failed to scope French locale: {error}"));
    let message = capture_panic_message(|| {
        let res: Result<(), &str> = Err("boom");
        assert_step_ok!(res);
    });
    assert!(message.contains("l'étape a renvoyé une erreur"));
    drop(guard);
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
fn assert_step_err_panics_on_ok_in_french() {
    let guard = ScopedLocalisation::new(&[langid!("fr")])
        .unwrap_or_else(|error| panic!("failed to scope French locale: {error}"));
    let message = capture_panic_message(|| {
        let res: Result<(), &str> = Ok(());
        let _ = assert_step_err!(res);
    });
    assert!(message.contains("l'étape a réussi"));
    drop(guard);
}

#[test]
fn assert_step_err_panics_on_ok_in_english() {
    let message = capture_panic_message(|| {
        let res: Result<(), &str> = Ok(());
        let _ = assert_step_err!(res);
    });
    assert_eq!(message, "step succeeded unexpectedly");
}
