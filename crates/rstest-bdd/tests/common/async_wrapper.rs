//! Async wrapper construction helpers for integration tests.

use rstest_bdd::{StepContext, StepFn, StepFuture};

/// Parameters for invoking a step function.
///
/// The explicit `'fixtures` lifetime is required because struct fields cannot
/// use placeholder lifetimes (like `StepContext<'_>`), and because async wrapper
/// signatures remain compatible with [`rstest_bdd::AsyncStepFn`], which
/// separates the lifetime of the borrowed [`StepContext`] from the lifetime of
/// the fixtures stored within it.
///
/// [`StepContext`]: rstest_bdd::StepContext
pub struct StepInvocationParams<'ctx, 'fixtures> {
    /// Mutable step context passed to the synchronous step.
    pub ctx: &'ctx mut StepContext<'fixtures>,
    /// Text passed to the step wrapper.
    pub text: &'ctx str,
    /// Optional docstring fixture passed to the step wrapper.
    pub docstring: Option<&'ctx str>,
    /// Optional data table fixture passed to the step wrapper.
    pub table: Option<&'ctx [&'ctx [&'ctx str]]>,
}

/// Wrap a synchronous step function (`StepFn`) into an async wrapper.
///
/// This helper is used to build explicit async wrappers for sync steps in
/// integration tests.
pub fn wrap_sync_step_as_async<'ctx>(
    sync_fn: StepFn,
    params: StepInvocationParams<'ctx, '_>,
) -> StepFuture<'ctx> {
    let StepInvocationParams {
        ctx,
        text,
        docstring,
        table,
    } = params;
    Box::pin(std::future::ready(sync_fn(ctx, text, docstring, table)))
}
