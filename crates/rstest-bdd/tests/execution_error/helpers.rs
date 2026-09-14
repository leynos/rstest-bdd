//! Test fixtures for execution-error formatting coverage.

use std::sync::Arc;

use rstest_bdd::{
    StepError,
    StepKeyword,
    execution::{ExecutionError, MissingFixtureDiagnostic, MissingFixturesDetails},
};

/// Helper to create a Skip error without message.
pub(super) fn skip_without_message() -> ExecutionError { ExecutionError::Skip { message: None } }
/// Helper to create a Skip error with message.
pub(super) fn skip_with_message(msg: &str) -> ExecutionError {
    ExecutionError::Skip {
        message: Some(msg.into()),
    }
}
/// Helper to create a `StepNotFound` error.
pub(super) fn step_not_found() -> ExecutionError {
    ExecutionError::StepNotFound {
        index: 3,
        keyword: StepKeyword::Given,
        text: "a user named Alice".into(),
        feature_path: "features/auth.feature".into(),
        scenario_name: "User login".into(),
    }
}

/// Parameter object for [`make_missing_fixtures`]; collects all
/// `MissingFixturesDetails` fields in one place so the builder function
/// stays under the argument-count threshold.
struct MissingFixturesSpec {
    pub step_pattern: &'static str,
    pub step_location: &'static str,
    pub required: Vec<&'static str>,
    pub missing: Vec<&'static str>,
    pub missing_requirements: Vec<MissingFixtureDiagnostic>,
    pub available: Vec<String>,
    pub has_suggestion: bool,
    pub feature_path: &'static str,
    pub scenario_name: &'static str,
}

/// Private builder: constructs a `MissingFixtures` `ExecutionError` from
/// the provided [`MissingFixturesSpec`], eliminating the repeated
/// `Arc::new(...)` scaffold shared by the two fixture-error factory helpers.
fn make_missing_fixtures(spec: MissingFixturesSpec) -> ExecutionError {
    ExecutionError::MissingFixtures(Arc::new(MissingFixturesDetails {
        step_pattern: spec.step_pattern.into(),
        step_location: spec.step_location.into(),
        required: spec.required,
        missing: spec.missing,
        missing_requirements: spec.missing_requirements,
        available: spec.available,
        has_suggestion: spec.has_suggestion,
        feature_path: spec.feature_path.into(),
        scenario_name: spec.scenario_name.into(),
    }))
}

/// Helper to create a `MissingFixtures` error.
pub(super) fn missing_fixtures() -> ExecutionError {
    make_missing_fixtures(MissingFixturesSpec {
        step_pattern: "a database connection",
        step_location: "tests/steps.rs:42",
        required: vec!["db", "cache"],
        missing: vec!["db"],
        missing_requirements: vec![MissingFixtureDiagnostic {
            name: "db",
            ty: "DbPool",
        }],
        available: vec!["cache".into(), "config".into()],
        has_suggestion: false,
        feature_path: "features/db.feature",
        scenario_name: "Database query",
    })
}

/// Helper to create a `MissingFixtures` error with harness guidance.
pub(super) fn missing_harness_fixture() -> ExecutionError {
    make_missing_fixtures(MissingFixturesSpec {
        step_pattern: "uses harness context",
        step_location: "tests/steps.rs:9",
        required: vec!["rstest_bdd_harness_context"],
        missing: vec!["rstest_bdd_harness_context"],
        missing_requirements: vec![MissingFixtureDiagnostic {
            name: "rstest_bdd_harness_context",
            ty: "AppContext",
        }],
        available: vec!["world".into()],
        has_suggestion: true,
        feature_path: "features/harness.feature",
        scenario_name: "Harness context",
    })
}

/// Helper to create a `HandlerFailed` error.
pub(super) fn handler_failed() -> ExecutionError {
    ExecutionError::HandlerFailed {
        index: 1,
        keyword: StepKeyword::When,
        text: "the user clicks submit".into(),
        error: Arc::new(StepError::ExecutionError {
            pattern: "the user clicks submit".into(),
            function: "click_submit".into(),
            message: "button not found".into(),
        }),
        feature_path: "features/form.feature".into(),
        scenario_name: "Form submission".into(),
    }
}
