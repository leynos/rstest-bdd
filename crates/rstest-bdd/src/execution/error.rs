//! Error types for step execution failures.

use std::sync::Arc;

use crate::{StepError, StepKeyword};

/// Error type for step execution failures.
///
/// This enum captures all failure modes during step execution, distinguishing
/// between control flow signals (skip requests) and actual errors (missing steps,
/// fixture validation failures, handler errors).
///
/// # Variants
///
/// - [`Skip`][Self::Skip]: Control flow signal indicating the step requested
///   skipping. This is not an error condition but a deliberate execution path.
/// - [`StepNotFound`][Self::StepNotFound]: The step pattern was not found in
///   the registry.
/// - [`MissingFixtures`][Self::MissingFixtures]: Required fixtures were not
///   available in the context.
/// - [`HandlerFailed`][Self::HandlerFailed]: The step handler returned an error.
///
/// # Examples
///
/// ```
/// use rstest_bdd::execution::ExecutionError;
///
/// let error = ExecutionError::Skip { message: Some("not implemented yet".into()) };
/// assert!(error.is_skip());
/// assert_eq!(error.skip_message(), Some("not implemented yet"));
/// ```
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum ExecutionError {
    /// Step requested to skip execution.
    Skip {
        /// Optional message explaining why the step was skipped.
        message: Option<String>,
    },
    /// Step pattern not found in the registry.
    StepNotFound {
        /// Zero-based step index.
        index: usize,
        /// The step keyword (Given, When, Then, etc.).
        keyword: StepKeyword,
        /// The step text that was not found.
        text: String,
        /// Path to the feature file.
        feature_path: String,
        /// Name of the scenario.
        scenario_name: String,
    },
    /// Required fixtures missing from context.
    ///
    /// The details are wrapped in `Arc` to reduce the size of `Result<T, ExecutionError>`.
    MissingFixtures(Arc<MissingFixturesDetails>),
    /// Step handler returned an error.
    HandlerFailed {
        /// Zero-based step index.
        index: usize,
        /// The step keyword (Given, When, Then, etc.).
        keyword: StepKeyword,
        /// The step text.
        text: String,
        /// The error returned by the handler, wrapped in Arc for Clone.
        error: Arc<StepError>,
        /// Path to the feature file.
        feature_path: String,
        /// Name of the scenario.
        scenario_name: String,
    },
}

/// Details about missing fixture errors.
///
/// This struct is separated from `ExecutionError::MissingFixtures` to allow
/// wrapping in `Arc`, reducing the overall size of `Result<T, ExecutionError>`.
#[derive(Debug, Clone)]
pub struct MissingFixturesDetails {
    /// The step pattern text.
    pub step_pattern: String,
    /// Source location of the step definition (`file:line`).
    pub step_location: String,
    /// List of all required fixture names.
    pub required: Vec<&'static str>,
    /// List of missing fixture names.
    pub missing: Vec<&'static str>,
    /// List of available fixture names in the context.
    pub available: Vec<String>,
    /// Path to the feature file.
    pub feature_path: String,
    /// Name of the scenario.
    pub scenario_name: String,
}

impl ExecutionError {
    /// Returns `true` if this error represents a skip request.
    ///
    /// Skip requests are control flow signals, not actual errors. Use this
    /// method to distinguish between errors that should fail a test and
    /// skip signals that should mark the test as skipped.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd::execution::ExecutionError;
    ///
    /// let skip = ExecutionError::Skip { message: None };
    /// assert!(skip.is_skip());
    ///
    /// let not_found = ExecutionError::StepNotFound {
    ///     index: 0,
    ///     keyword: rstest_bdd::StepKeyword::Given,
    ///     text: "missing".into(),
    ///     feature_path: "test.feature".into(),
    ///     scenario_name: "test".into(),
    /// };
    /// assert!(!not_found.is_skip());
    /// ```
    #[must_use]
    pub fn is_skip(&self) -> bool {
        matches!(self, Self::Skip { .. })
    }

    /// Returns the skip message if this is a skip error.
    ///
    /// Returns `None` if this is not a skip error, or if the skip has no
    /// message.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd::execution::ExecutionError;
    ///
    /// let skip_with_msg = ExecutionError::Skip { message: Some("reason".into()) };
    /// assert_eq!(skip_with_msg.skip_message(), Some("reason"));
    ///
    /// let skip_no_msg = ExecutionError::Skip { message: None };
    /// assert_eq!(skip_no_msg.skip_message(), None);
    ///
    /// let not_skip = ExecutionError::StepNotFound {
    ///     index: 0,
    ///     keyword: rstest_bdd::StepKeyword::Given,
    ///     text: "missing".into(),
    ///     feature_path: "test.feature".into(),
    ///     scenario_name: "test".into(),
    /// };
    /// assert_eq!(not_skip.skip_message(), None);
    /// ```
    #[must_use]
    pub fn skip_message(&self) -> Option<&str> {
        match self {
            Self::Skip { message } => message.as_deref(),
            _ => None,
        }
    }
}

impl ExecutionError {
    /// Render the error message using the provided Fluent loader.
    #[must_use]
    pub fn format_with_loader(&self, loader: &crate::FluentLanguageLoader) -> String {
        match self {
            Self::Skip { message } => {
                crate::localization::message_with_loader(loader, "execution-error-skip", |args| {
                    args.set(
                        "has_message",
                        if message.is_some() { "yes" } else { "no" }.to_string(),
                    );
                    args.set("message", message.clone().unwrap_or_default());
                })
            }
            Self::StepNotFound {
                index,
                keyword,
                text,
                feature_path,
                scenario_name,
            } => crate::localization::message_with_loader(
                loader,
                "execution-error-step-not-found",
                |args| {
                    args.set("index", index.to_string());
                    args.set("keyword", keyword.as_str().to_string());
                    args.set("text", text.clone());
                    args.set("feature_path", feature_path.clone());
                    args.set("scenario_name", scenario_name.clone());
                },
            ),
            Self::MissingFixtures(details) => crate::localization::message_with_loader(
                loader,
                "execution-error-missing-fixtures",
                |args| {
                    args.set("step_pattern", details.step_pattern.clone());
                    args.set("step_location", details.step_location.clone());
                    args.set("required", details.required.join(", "));
                    args.set("missing", details.missing.join(", "));
                    args.set("available", details.available.join(", "));
                    args.set("feature_path", details.feature_path.clone());
                    args.set("scenario_name", details.scenario_name.clone());
                },
            ),
            Self::HandlerFailed {
                index,
                keyword,
                text,
                error,
                feature_path,
                scenario_name,
            } => crate::localization::message_with_loader(
                loader,
                "execution-error-handler-failed",
                |args| {
                    args.set("index", index.to_string());
                    args.set("keyword", keyword.as_str().to_string());
                    args.set("text", text.clone());
                    args.set("error", error.to_string());
                    args.set("feature_path", feature_path.clone());
                    args.set("scenario_name", scenario_name.clone());
                },
            ),
        }
    }
}

impl std::fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = crate::localization::with_loader(|loader| self.format_with_loader(loader));
        f.write_str(&message)
    }
}

impl std::error::Error for ExecutionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::HandlerFailed { error, .. } => Some(error.as_ref()),
            _ => None,
        }
    }
}
