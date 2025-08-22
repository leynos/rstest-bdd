//! Unit tests for `StepError` formatting

use rstest_bdd::StepError;

#[test]
fn missing_fixture_formats() {
    let err = StepError::MissingFixture {
        name: "item".into(),
        ty: "u32".into(),
        step: "my_step".into(),
    };
    assert_eq!(
        err.to_string(),
        "Missing fixture 'item' of type 'u32' for step function 'my_step'"
    );
}

#[test]
fn execution_error_formats() {
    let err = StepError::ExecutionError {
        step: "exec".into(),
        message: "boom".into(),
    };
    assert_eq!(
        err.to_string(),
        "Execution error in step function 'exec': boom"
    );
}

#[test]
fn panic_error_formats() {
    let err = StepError::PanicError {
        pattern: "pattern".into(),
        function: "func".into(),
        message: "payload".into(),
    };
    assert_eq!(
        err.to_string(),
        "Panic in step 'pattern', function 'func': payload"
    );
}

#[test]
fn panic_error_formats_non_string_payload() {
    let payload: Box<dyn std::any::Any + Send> = Box::new(42u8);
    let message = rstest_bdd::panic_message(payload.as_ref());
    let err = StepError::PanicError {
        pattern: "pattern".into(),
        function: "func".into(),
        message,
    };
    assert_eq!(
        err.to_string(),
        "Panic in step 'pattern', function 'func': 42",
    );
}
