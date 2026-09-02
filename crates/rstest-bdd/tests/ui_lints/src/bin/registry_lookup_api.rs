//! Clippy UI fixture for the public registry metadata lookup API.
//!
//! The fixture compiles the canonical metadata lookup pair under `-D warnings`
//! and pins the ambiguity contract: both helpers return
//! `Result<Option<ResolvedStep>, StepLookupError>`, so a tie between equally
//! specific definitions is reported rather than silently resolving to one of
//! them. Projecting through `as_step()`, `Deref`, and the individual handler
//! and metadata fields is exercised at the same time.

#![deny(warnings)]

use rstest_bdd::{
    AsyncStepFn,
    ResolvedStep,
    Step,
    StepExecutionMode,
    StepFn,
    StepKeyword,
    StepLookupError,
    StepText,
    find_step_with_metadata,
    lookup_step_with_metadata,
};

fn keyword_of(step: &ResolvedStep) -> StepKeyword { step.as_step().keyword }

/// Collapse either lookup's error into one message, proving both are fallible.
fn ambiguity_message(result: Result<Option<ResolvedStep>, StepLookupError>) -> Option<String> {
    result.err().map(|error| error.to_string())
}

fn main() {
    let keyword = StepKeyword::Given;
    let exact = lookup_step_with_metadata(keyword, "a registered step".into());
    let fuzzy = find_step_with_metadata(keyword, StepText::from("a registered step"));

    // Each lookup is fallible on its own, so the pair cannot be `or`-chained on
    // `Option`; the ambiguity must be handled before the optional result.
    let exact = exact.expect("exact lookup should be unambiguous");
    let fuzzy = fuzzy.expect("fuzzy lookup should be unambiguous");
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
        ambiguity_message,
    );
}
