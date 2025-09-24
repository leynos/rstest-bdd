//! Shared fixtures and helpers for `StepError` behaviour tests.

use rstest_bdd::{StepContext, StepError, StepKeyword};
use rstest_bdd_macros::{given, then, when};
use std::fmt;

type StepResult<T> = Result<T, &'static str>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FancyValue(pub u16);

#[derive(Debug, PartialEq, Eq)]
pub struct FancyError(&'static str);

impl fmt::Display for FancyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

#[given("a failing step")]
fn failing_step() -> Result<(), String> {
    Err("boom".into())
}

#[given("a panicking step")]
fn panicking_step() -> Result<(), String> {
    panic!("kaboom")
}

#[given("a non-string panicking step")]
fn non_string_panicking_step() -> Result<(), String> {
    std::panic::panic_any(123_i32)
}

#[given("a successful step")]
fn successful_step() {}

#[when("a failing when step")]
fn failing_when_step() -> Result<(), String> {
    Err("when boom".into())
}

#[then("a failing then step")]
fn failing_then_step() -> Result<(), String> {
    Err("then boom".into())
}

#[given("an alias error step")]
fn alias_error_step() -> StepResult<()> {
    Err("alias boom")
}

#[given("a fallible unit step succeeds")]
#[expect(
    clippy::unnecessary_wraps,
    reason = "step intentionally returns Result to exercise IntoStepResult"
)]
fn fallible_unit_step_succeeds() -> Result<(), FancyError> {
    Ok(())
}

#[given("a fallible unit step fails")]
fn fallible_unit_step_fails() -> Result<(), FancyError> {
    Err(FancyError("unit failure"))
}

#[given("a fallible value step succeeds")]
#[expect(
    clippy::unnecessary_wraps,
    reason = "step intentionally returns Result to exercise IntoStepResult"
)]
fn fallible_value_step_succeeds() -> Result<FancyValue, FancyError> {
    Ok(FancyValue(99))
}

#[given("a fallible value step fails")]
fn fallible_value_step_fails() -> Result<FancyValue, FancyError> {
    Err(FancyError("value failure"))
}

#[given("a step requiring a table")]
#[expect(clippy::needless_pass_by_value, reason = "step consumes the table")]
fn step_needing_table(datatable: Vec<Vec<String>>) {
    let _ = datatable;
}

#[given("a step requiring a docstring")]
#[expect(clippy::needless_pass_by_value, reason = "step consumes docstring")]
fn step_needing_docstring(docstring: String) {
    let _ = docstring;
}

#[given("number {value}")]
fn parse_number(value: u32) {
    let _ = value;
}

#[given("no placeholders")]
fn missing_capture(value: u32) {
    let _ = value;
}

#[derive(Debug)]
pub struct StepInvocation<'a> {
    keyword: StepKeyword,
    step_pattern: &'a str,
    step_text: &'a str,
    pub(crate) docstring: Option<&'a str>,
    pub(crate) datatable: Option<&'a [&'a [&'a str]]>,
}

impl<'a> StepInvocation<'a> {
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
}

/// Invoke a registered step and return its optional payload.
///
/// # Errors
/// Returns a [`StepError`] when the step fails to execute.
///
/// # Panics
/// Panics when the step pattern has not been registered in the global registry.
pub fn invoke_step(
    invocation: &StepInvocation<'_>,
) -> Result<Option<Box<dyn std::any::Any>>, StepError> {
    let ctx = StepContext::default();
    let step_fn = rstest_bdd::lookup_step(invocation.keyword, invocation.step_pattern.into())
        .unwrap_or_else(|| panic!("step '{}' not found in registry", invocation.step_pattern));
    step_fn(
        &ctx,
        invocation.step_text,
        invocation.docstring,
        invocation.datatable,
    )
}
