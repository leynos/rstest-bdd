//! Common helper functions for behavioural tests.

use rstest_bdd::{StepContext, StepError, StepExecution};

/// No-op step wrapper matching the `StepFn` signature.
///
/// # Examples
/// ```rust
/// use rstest_bdd::{StepKeyword, step, find_step, StepContext};
///
/// step!(StepKeyword::Given, "example", noop_wrapper, &[]);
/// let runner = find_step(StepKeyword::Given, "example".into()).unwrap();
/// let mut ctx = StepContext::default();
/// match runner(&mut ctx, "example", None, None) {
///     Ok(rstest_bdd::StepExecution::Continue { .. }) => {}
///     other => panic!("unexpected step outcome: {other:?}"),
/// }
/// ```
#[expect(
    clippy::unnecessary_wraps,
    reason = "wrapper must match StepFn signature"
)]
pub fn noop_wrapper(
    ctx: &mut StepContext<'_>,
    _text: &str,
    _docstring: Option<&str>,
    _table: Option<&[&[&str]]>,
) -> Result<StepExecution, StepError> {
    let _ = ctx;
    Ok(StepExecution::from_value(None))
}
