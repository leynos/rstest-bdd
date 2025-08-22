//! Unit tests for `StepError` formatting

use rstest_bdd::StepError;

#[test]
fn missing_fixture_formats() {
    let err = StepError::MissingFixture {
        name: "item".to_string(),
        ty: "u32".to_string(),
        step: "my_step".to_string(),
    };
    assert_eq!(
        err.to_string(),
        "Missing fixture 'item' of type 'u32' for step function 'my_step'"
    );
}

#[test]
fn execution_error_formats() {
    let err = StepError::ExecutionError {
        step: "exec".to_string(),
        message: "boom".to_string(),
    };
    assert_eq!(
        err.to_string(),
        "Execution error in step function 'exec': boom"
    );
}

#[test]
fn panic_error_formats() {
    let err = StepError::PanicError {
        pattern: "pattern".to_string(),
        function: "func".to_string(),
        message: "payload".to_string(),
    };
    assert_eq!(
        err.to_string(),
        "Panic in step 'pattern', function 'func': payload"
    );
}
