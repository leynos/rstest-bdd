//! Unit tests for `assert_step_ok!` and `assert_step_err!`

use rstest_bdd::{assert_step_err, assert_step_ok};

#[test]
fn assert_step_ok_unwraps_result() {
    let res: Result<(), &str> = Ok(());
    assert_step_ok!(res);
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
#[should_panic(expected = "step succeeded unexpectedly")]
fn assert_step_err_panics_on_ok() {
    let res: Result<(), &str> = Ok(());
    let _ = assert_step_err!(res);
}
