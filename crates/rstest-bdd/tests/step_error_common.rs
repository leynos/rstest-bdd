//! Shared helpers for step error behavioural tests.

use rstest_bdd::{StepContext, StepError, StepExecution, StepKeyword};
use rstest_bdd_macros::{given, then, when};
use std::fmt;

/// Simulate a step function that returns an execution failure.
///
/// # Errors
/// Always returns an error describing the failure.
#[given("a failing step")]
pub fn failing_step() -> Result<(), String> {
    Err("boom".into())
}

/// Simulate a step function that panics instead of returning an error.
///
/// # Panics
/// Always panics with a fixed message to exercise panic handling.
///
/// # Errors
/// This function never returns an error because it panics.
#[given("a panicking step")]
pub fn panicking_step() -> Result<(), String> {
    panic!("kaboom")
}

/// Simulate a panic that carries a non-string payload.
///
/// # Panics
/// Always panics with an integer payload to exercise panic formatting.
///
/// # Errors
/// This function never returns an error because it panics.
#[given("a non-string panicking step")]
pub fn non_string_panicking_step() -> Result<(), String> {
    std::panic::panic_any(123_i32)
}

/// Trivial step that succeeds without returning data.
#[given("a successful step")]
pub fn successful_step() {}

/// Simulate a failing `when` step.
///
/// # Errors
/// Always returns an error describing the failure.
#[when("a failing when step")]
pub fn failing_when_step() -> Result<(), String> {
    Err("when boom".into())
}

/// Simulate a failing `then` step.
///
/// # Errors
/// Always returns an error describing the failure.
#[then("a failing then step")]
pub fn failing_then_step() -> Result<(), String> {
    Err("then boom".into())
}

/// Convenience alias for steps that intentionally use a `Result`.
pub type StepResult<T> = Result<T, &'static str>;

/// Lightweight newtype used to exercise value propagation in step tests.
///
/// # Examples
/// ```ignore
/// use crate::step_error_common::FancyValue;
///
/// let value = FancyValue(7);
/// assert_eq!(value.0, 7);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FancyValue(pub u16);

/// Error wrapper surfaced by simulated steps during behavioural tests.
///
/// # Examples
/// ```ignore
/// use crate::step_error_common::FancyError;
///
/// let err = FancyError("broken");
/// assert_eq!(err.to_string(), "broken");
/// ```
#[derive(Debug, PartialEq, Eq)]
pub struct FancyError(pub &'static str);

impl fmt::Display for FancyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

/// Step that surfaces an alias error to exercise step wrapper normalisation.
///
/// # Errors
/// Always returns an error describing the failure.
#[given("an alias error step", result)]
pub fn alias_error_step() -> StepResult<()> {
    Err("alias boom")
}

/// Step that succeeds while returning a `Result` payload.
///
/// # Errors
/// Returns an error only if the simulation fails; in these tests it always
/// succeeds.
#[given("a fallible unit step succeeds")]
pub fn fallible_unit_step_succeeds() -> Result<(), FancyError> {
    "42".parse::<u32>()
        .map(|_| ())
        .map_err(|_| FancyError("unit failure"))
}

/// Step that fails while returning a custom error.
///
/// # Errors
/// Always returns a `FancyError` describing the failure.
#[given("a fallible unit step fails")]
pub fn fallible_unit_step_fails() -> Result<(), FancyError> {
    "invalid"
        .parse::<u32>()
        .map(|_| ())
        .map_err(|_| FancyError("unit failure"))
}

/// Step that returns a value inside a `Result`.
///
/// # Errors
/// Returns an error only if the simulation fails; in these tests it always
/// succeeds.
#[given("a fallible value step succeeds")]
pub fn fallible_value_step_succeeds() -> Result<FancyValue, FancyError> {
    "99".parse::<u16>()
        .map(FancyValue)
        .map_err(|_| FancyError("value failure"))
}

/// Step that fails while attempting to return a value.
///
/// # Errors
/// Always returns a `FancyError` describing the failure.
#[given("a fallible value step fails")]
pub fn fallible_value_step_fails() -> Result<FancyValue, FancyError> {
    "invalid"
        .parse::<u16>()
        .map(FancyValue)
        .map_err(|_| FancyError("value failure"))
}

/// Step that consumes an incoming data table.
#[given("a step requiring a table")]
pub fn step_needing_table(datatable: Vec<Vec<String>>) {
    let _ = datatable;
}

/// Step that consumes an incoming docstring.
#[given("a step requiring a docstring")]
pub fn step_needing_docstring(docstring: String) {
    let _ = docstring;
}

/// Step implementation that raises a skip request during execution.
///
/// # Examples
/// ```ignore
/// use rstest_bdd_macros::given;
///
/// #[given("a skip request step")]
/// fn skip_step() {
///     rstest_bdd::skip!("skip for documentation");
/// }
/// ```
#[given("a skip request step")]
pub fn skip_request_step() {
    rstest_bdd::skip!("behavioural skip test");
}

/// Step definition that captures a numeric placeholder for verification.
///
/// # Examples
/// ```ignore
/// use rstest_bdd_macros::given;
///
/// #[given("number {value}")]
/// fn parse_number(value: u32) {
///     assert!(value >= 0);
/// }
/// ```
#[given("number {value}")]
pub fn parse_number(value: u32) {
    let _ = value;
}

/// Step definition deliberately ignoring its captured argument to test errors.
///
/// # Examples
/// ```ignore
/// use rstest_bdd_macros::given;
///
/// #[given("no placeholders")]
/// fn missing_capture(value: u32) {
///     let _ = value;
/// }
/// ```
#[given("no placeholders")]
pub fn missing_capture(value: u32) {
    let _ = value;
}

/// In-memory description of a step invocation used by the behavioural tests.
///
/// # Examples
/// ```ignore
/// use crate::step_error_common::{StepInvocation, StepKeyword};
///
/// let invocation = StepInvocation::new(StepKeyword::Given, "pattern", "text");
/// assert_eq!(invocation.step_text, "text");
/// ```
#[derive(Debug)]
pub struct StepInvocation<'a> {
    /// Step keyword (`Given`, `When`, or `Then`) selecting the registered step.
    pub keyword: StepKeyword,
    /// Pattern used to locate the registered step implementation.
    pub step_pattern: &'a str,
    /// Concrete text passed to the step function during execution.
    pub step_text: &'a str,
    /// Optional docstring forwarded to the step function.
    pub docstring: Option<&'a str>,
    /// Optional data table forwarded to the step function.
    pub datatable: Option<&'a [&'a [&'a str]]>,
}

impl<'a> StepInvocation<'a> {
    /// Construct a new invocation description for the provided keyword.
    #[must_use]
    pub fn new(keyword: StepKeyword, step_pattern: &'a str, step_text: &'a str) -> Self {
        Self {
            keyword,
            step_pattern,
            step_text,
            docstring: None,
            datatable: None,
        }
    }

    /// Attach a docstring to the invocation.
    #[must_use]
    pub fn with_docstring(mut self, docstring: &'a str) -> Self {
        self.docstring = Some(docstring);
        self
    }

    /// Attach a data table to the invocation.
    #[must_use]
    pub fn with_datatable(mut self, datatable: &'a [&'a [&'a str]]) -> Self {
        self.datatable = Some(datatable);
        self
    }
}

/// Invoke a registered step and capture its result.
///
/// # Errors
/// Returns any [`StepError`] surfaced by the registered step implementation.
///
/// # Panics
/// Panics if the requested step has not been registered in the global registry.
pub fn invoke_step(invocation: &StepInvocation<'_>) -> Result<StepExecution, StepError> {
    let mut ctx = StepContext::default();
    let step_fn = rstest_bdd::lookup_step(invocation.keyword, invocation.step_pattern.into())
        .unwrap_or_else(|| panic!("step '{}' not found in registry", invocation.step_pattern));
    step_fn(
        &mut ctx,
        invocation.step_text,
        invocation.docstring,
        invocation.datatable,
    )
}
