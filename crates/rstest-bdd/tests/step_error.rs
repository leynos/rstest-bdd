//! Unit tests for `StepError` display formatting

use rstest::rstest;
use rstest_bdd::StepError;

#[rstest]
#[case(
    StepError::MissingFixture {
        name: "n".into(),
        ty: "u32".into(),
        step: "s".into(),
    },
    "Missing fixture 'n' of type 'u32' for step function 's'",
)]
#[case(
    StepError::ExecutionError {
        pattern: "p".into(),
        function: "f".into(),
        message: "m".into(),
    },
    "Error executing step 'p' via function 'f': m",
)]
#[case(
    StepError::PanicError {
        pattern: "p".into(),
        function: "f".into(),
        message: "boom".into(),
    },
    "Panic in step 'p', function 'f': boom",
)]
fn step_error_display_formats(#[case] err: StepError, #[case] expected: &str) {
    assert_eq!(err.to_string(), expected);
}
