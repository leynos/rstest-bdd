//! The panic boundary around a step handler invocation (D11).
//!
//! `run_scenario` has a documented contract: every step outcome reaches the
//! caller through the returned `ScenarioOutcome`, and the function does not
//! panic (Constraint 3, ADR-018-TR2). For a step registered through `#[given]`
//! and its siblings that contract is satisfied one level down, by the
//! macro-generated wrapper's own `catch_unwind`
//! (`crates/rstest-bdd-macros/src/codegen/wrapper/emit/assembly/mod.rs`). It is
//! *not* satisfied for a step registered through the raw `step!` form, which
//! stores the handler as a bare function pointer and generates no wrapper — and
//! that form is ADR-018's own extension story, so a runner that relied on the
//! wrapper would be relying on something a legitimate registration path does
//! not have.
//!
//! This module is that boundary.
//!
//! # The mapping is the wrapper's, deliberately
//!
//! [`guarded`] folds whatever escapes back into the *handler-shaped* result the
//! execution layer already knows how to carry, rather than into a
//! `ExecutionError` of its own. The reason is not economy of code: it makes the
//! panic path and the ordinary path converge on the single mapping in
//! `handle_step_result`, so there is no second place for a panic to be
//! classified differently from an `Err` the handler returned on purpose.
//!
//! The mapping itself is the macro wrapper's, downcast to `SkipRequest`
//! included, and the resemblance is intentional. A step's behaviour should not
//! depend on which of two legitimate registration forms the author chose, and
//! copying the wrapper's arms is what makes that true rather than merely
//! intended. `runner_panics.rs` asserts it from the outside, by requiring the
//! wrapped and unwrapped cases to classify identically.
//!
//! # Why the drivers do not do this
//!
//! The boundary lives here, at the one call every step invocation passes
//! through, rather than in `engine::drive_sync`. `engine::policy`'s module note
//! gives the reason for that whole split: the two drivers differ only in how
//! they await a step, so anything they would otherwise both decide exists twice
//! and can drift. A panic boundary is exactly such a decision. Putting it here
//! also makes the contract true where it is stated: `execute_step` documents
//! that it returns `ExecutionError` for all failure cases, and a panic was the
//! one failure case that did not.
//!
//! # Why this does not catch a panicking destructor
//!
//! `catch_unwind` cannot: a destructor that panics *while* another panic
//! unwinds aborts the process before any handler runs, and no amount of nesting
//! changes that. D11 names three paths the runner owns, and this module closes
//! the step-invocation one; value destructors run by cleanup are closed by the
//! `catch_unwind` already inside `runner::scope`'s `CleanupGuard`. That is why
//! this file does not appear to cover the case D11's rationale leads with.

use std::{
    any::Any,
    panic::{AssertUnwindSafe, catch_unwind},
};

use crate::{Step, StepError, StepExecution, panic_message, skip::SkipRequest};

/// Invoke a step handler behind the runner's panic boundary.
///
/// Returns a `Result<StepExecution, StepError>` in exactly the shape an
/// unguarded handler call would, so the caller passes it to
/// `handle_step_result` unchanged:
///
/// - the handler's own result, when it returned one;
/// - `Ok(StepExecution::Skipped { .. })` when the payload was a `SkipRequest`, which is how `skip!`
///   propagates out of a step; or
/// - `Err(StepError::PanicError { .. })` for anything else, which
///   [`FailureKind::of`](crate::runner::FailureKind::of) classifies as
///   [`Panic`](crate::runner::FailureKind::Panic).
///
/// `AssertUnwindSafe` is required because the closure borrows the
/// `StepContext`, which holds interior-mutable cells and is not `UnwindSafe`.
/// Asserting it is sound for the same reason the macro wrapper asserts it: a
/// panic part-way through a handler leaves *fewer* inserted values than a
/// completed run, never a half-visible one, because `insert_value` either
/// completes or does not.
pub(super) fn guarded(
    step: &Step,
    invoke: impl FnOnce() -> Result<StepExecution, StepError>,
) -> Result<StepExecution, StepError> {
    match catch_unwind(AssertUnwindSafe(invoke)) {
        Ok(result) => result,
        Err(payload) => from_payload(step, payload),
    }
}

/// Map a caught panic payload into the handler-shaped result.
///
/// Split from [`guarded`] because the async path cannot use `guarded` at all:
/// a genuine `async` body must keep yielding at its awaits, so its unwinds are
/// caught per poll by `catch_unwind_future` rather than around a synchronous
/// call. Both paths end here, which is what keeps the two registration forms
/// from disagreeing about what a panic means.
///
/// Takes the payload by value because `SkipRequest::into_message` consumes the
/// request and there is no borrowed accessor — deliberately, since a skip
/// happens once and the message moves out of it. The `Err` arm needs only a
/// borrow, which the by-value payload still provides.
pub(super) fn from_payload(
    step: &Step,
    payload: Box<dyn Any + Send>,
) -> Result<StepExecution, StepError> {
    match payload.downcast::<SkipRequest>() {
        // A skip is control flow, not a failure: the step asked for this, and
        // `skip!` documents the mechanism.
        Ok(request) => Ok(StepExecution::skipped(request.into_message())),
        Err(payload) => Err(panicked(step, payload.as_ref())),
    }
}

/// Render a caught panic as the step error the runner reports.
///
/// `function` is filled with `file:line` rather than a function name, and the
/// reason is structural rather than cosmetic: a `Step` records where a step was
/// defined, not what its handler was called, so no name is recoverable here.
/// The wrapper fills the field from `stringify!`, which it can do only because
/// it *is* the generated code. A raw registration has no equivalent, and a
/// fabricated name would be worse than an absent one. `file:line` is the
/// crate's existing answer to this question — `MissingFixturesDetails` names a
/// step the same way, for the same reason — and unlike a guessed identifier it
/// is actionable, because it points at the line a reader has to open.
fn panicked(step: &Step, payload: &(dyn Any + Send)) -> StepError {
    StepError::PanicError {
        pattern: step.pattern.as_str().to_owned(),
        function: format!("{}:{}", step.file, step.line),
        message: panic_message(payload),
    }
}
