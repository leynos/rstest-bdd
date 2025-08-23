//! Unit tests for `StepError` display formatting

use rstest_bdd::StepError;

#[test]
fn missing_fixture_formats() {
    let err = StepError::MissingFixture {
        name: "n".into(),
        ty: "u32".into(),
        step: "s".into(),
    };
    assert_eq!(
        err.to_string(),
        "Missing fixture 'n' of type 'u32' for step function 's'"
    );
}

#[test]
fn execution_error_formats() {
    let err = StepError::ExecutionError {
        pattern: "p".into(),
        function: "f".into(),
        message: "m".into(),
    };
    assert_eq!(
        err.to_string(),
        "Error executing step 'p' via function 'f': m"
    );
}

#[test]
fn panic_error_formats() {
    let err = StepError::PanicError {
        pattern: "p".into(),
        function: "f".into(),
        message: "boom".into(),
    };
    assert_eq!(err.to_string(), "Panic in step 'p', function 'f': boom");
}
