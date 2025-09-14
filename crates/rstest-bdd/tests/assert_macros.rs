//! Unit tests for `assert_step_ok!` and `assert_step_err!`

use rstest_bdd::{assert_step_err, assert_step_ok};

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
#[should_panic(expected = "step returned error")]
fn assert_step_ok_panics_on_err() {
    let res: Result<(), &str> = Err("boom");
    assert_step_ok!(res);
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
#[should_panic(expected = "does not contain")]
fn assert_step_err_panics_when_substring_absent() {
    let res: Result<(), &str> = Err("boom");
    let _ = assert_step_err!(res, "absent");
}

#[test]
#[should_panic(expected = "step succeeded unexpectedly")]
fn assert_step_err_panics_on_ok() {
    let res: Result<(), &str> = Ok(());
    let _ = assert_step_err!(res);
}
