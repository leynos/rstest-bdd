//! Clippy UI fixture for the public registry metadata lookup API.
//!
//! The fixture compiles the canonical metadata lookup pair, the `ResolvedStep`
//! dereference and explicit `as_step()` projection, and each handler and
//! metadata field under `-D warnings`.

#![deny(warnings)]

use rstest_bdd::{
    AsyncStepFn,
    ResolvedStep,
    Step,
    StepExecutionMode,
    StepFn,
    StepKeyword,
    StepText,
    find_step_with_metadata,
    lookup_step_with_metadata,
};

fn keyword_of(step: &ResolvedStep) -> StepKeyword { step.as_step().keyword }

fn main() {
    let keyword = StepKeyword::Given;
    let exact = lookup_step_with_metadata(keyword, "a registered step".into());
    let fuzzy = find_step_with_metadata(keyword, StepText::from("a registered step"));
    let Some(step) = exact.or(fuzzy) else {
        return;
    };

    // `as_step` yields the static reference; `Deref` exposes the same fields.
    let step_ref: &'static Step = step.as_step();
    let run: StepFn = step.run;
    let run_async: AsyncStepFn = step.run_async;
    let execution_mode: StepExecutionMode = step.execution_mode;

    let _ = (
        step_ref,
        keyword_of(&step),
        run,
        run_async,
        execution_mode,
        step.fixtures,
        step.file,
        step.line,
    );
}