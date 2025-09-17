//! Common helper functions for behavioural tests.

use rstest_bdd::{StepContext, StepError};

/// No-op step wrapper matching the `StepFn` signature.
///
/// # Examples
/// ```rust
/// use rstest_bdd::{StepKeyword, step, find_step, StepContext};
///
/// step!(StepKeyword::Given, "example", noop_wrapper, &[]);
/// let runner = find_step(StepKeyword::Given, "example".into()).unwrap();
/// runner(&StepContext::default(), "example", None, None).unwrap();
/// ```
#[expect(
    clippy::unnecessary_wraps,
    reason = "wrapper must match StepFn signature"
)]
pub fn noop_wrapper(
    ctx: &StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<Option<Box<dyn std::any::Any>>, StepError> {
    let _ = ctx;
    Ok(None)
}
