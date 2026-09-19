//! The synthetic inputs every decision-layer test is built from.
//!
//! One home for the fixtures, so the three test modules below differ only in
//! what they assert. Every record here is built by hand: none of it touches the
//! registry or a [`StepContext`](crate::StepContext), which is what makes the
//! decision layer checkable on its own (LEM-1).

use std::sync::Arc;

use crate::{
    StepError,
    StepKeyword,
    execution::ExecutionError,
    runner::{
        engine::policy::{SkipPolicy, Terminal},
        outcome::StepOutcome,
        source::SourceLocation,
    },
};

/// The source location every synthetic record is given.
pub(super) fn location() -> SourceLocation { SourceLocation::new_static("notes/demo.md", 12, None) }

/// A `StepNotFound` failure; the cheapest non-skip `Err` to build.
pub(super) fn not_found() -> ExecutionError {
    ExecutionError::StepNotFound {
        index: 0,
        keyword: StepKeyword::Given,
        text: "an undefined step".to_owned(),
        feature_path: "notes/demo.md".to_owned(),
        scenario_name: "demo".to_owned(),
    }
}

/// A `MissingFixtures` failure, the second non-skip class.
pub(super) fn missing_fixtures() -> ExecutionError {
    ExecutionError::MissingFixtures(Arc::new(crate::execution::MissingFixturesDetails {
        step_pattern: "a calculator".to_owned(),
        step_location: "steps.rs:1".to_owned(),
        required: vec!["calc"],
        missing: vec!["calc"],
        missing_requirements: Vec::new(),
        available: Vec::new(),
        has_suggestion: false,
        feature_path: "notes/demo.md".to_owned(),
        scenario_name: "demo".to_owned(),
    }))
}

/// A `HandlerFailed` failure, the third non-skip class.
pub(super) fn handler_failed() -> ExecutionError {
    ExecutionError::HandlerFailed {
        index: 0,
        keyword: StepKeyword::Given,
        text: "a calculator".to_owned(),
        error: Arc::new(StepError::ExecutionError {
            pattern: "a calculator".to_owned(),
            function: "a_calculator".to_owned(),
            message: "boom".to_owned(),
        }),
        feature_path: "notes/demo.md".to_owned(),
        scenario_name: "demo".to_owned(),
    }
}

/// A skip error carrying the supplied reason.
pub(super) fn skip(message: Option<&str>) -> ExecutionError {
    ExecutionError::Skip {
        message: message.map(str::to_owned),
    }
}

/// A passing record at `index`.
pub(super) fn passed(index: usize) -> StepOutcome {
    StepOutcome::passed(
        index,
        StepKeyword::Given,
        "a calculator",
        Some(&location()),
        None,
    )
}

/// A bypassed record at `index`.
pub(super) fn bypassed(index: usize) -> StepOutcome {
    StepOutcome::bypassed(index, StepKeyword::Given, "a calculator", Some(&location()))
}

/// A skipped record at `index`.
pub(super) fn skipped(index: usize) -> StepOutcome {
    StepOutcome::skipped(
        index,
        StepKeyword::Given,
        "a pending step",
        Some(&location()),
        Some("waiting on upstream".to_owned()),
    )
}

/// A failed record at `index`.
pub(super) fn failed(index: usize) -> StepOutcome {
    StepOutcome::failed(
        index,
        StepKeyword::Given,
        "an undefined step",
        Some(&location()),
        not_found(),
    )
}

/// A policy under which a scenario-level skip is permitted.
///
/// Derived through [`SkipPolicy::resolve`] rather than written as a struct
/// literal. The fields are private to the engine, so a literal is possible
/// here, and that is exactly the hazard: a literal spells a *pre*-resolution
/// shape (`allow_skipped: false, fail_on_skipped: false` reads as "permissive"
/// but is not what a resolved permissive policy holds), and a test that
/// constructs one asserts post-resolution invariants against a value the
/// production code never builds. Resolving keeps these constants honest by
/// construction, and states the plan-side flag each one stands for.
pub(super) const PERMITTED: SkipPolicy = SkipPolicy::resolve(true, true);

/// A policy under which a scenario-level skip is refused.
pub(super) const REFUSED: SkipPolicy = SkipPolicy::resolve(false, true);

/// A scope-level policy that permits skipping even when the plan does not.
///
/// The plan refuses skipping and the scope overrides it, so the effective
/// `allow_skipped` is `true` — the case the literal got wrong.
pub(super) const PERMISSIVE: SkipPolicy = SkipPolicy::resolve(false, false);

/// A terminal skip at index 0, as a driver would build it.
pub(super) fn skip_terminal(message: Option<&str>) -> Terminal {
    Terminal::Skip {
        index: 0,
        message: message.map(str::to_owned),
        source: Some(location()),
    }
}
