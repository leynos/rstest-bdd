//! Shared scenario runner request and metadata types.

/// Scenario metadata provided to harness adapters.
///
/// # Examples
///
/// ```
/// use rstest_bdd_harness::ScenarioMetadata;
///
/// let metadata = ScenarioMetadata::new(
///     "tests/features/login.feature",
///     "Successful login",
///     12,
///     vec!["@smoke".to_string()],
/// );
/// assert_eq!(metadata.feature_path(), "tests/features/login.feature");
/// assert_eq!(metadata.scenario_name(), "Successful login");
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScenarioMetadata {
    feature_path: String,
    scenario_name: String,
    scenario_line: u32,
    tags: Vec<String>,
}

impl ScenarioMetadata {
    /// Creates metadata for one scenario run.
    #[must_use]
    pub fn new(
        feature_path: impl Into<String>,
        scenario_name: impl Into<String>,
        scenario_line: u32,
        tags: Vec<String>,
    ) -> Self {
        Self {
            feature_path: feature_path.into(),
            scenario_name: scenario_name.into(),
            scenario_line,
            tags,
        }
    }

    /// Returns the feature path.
    #[must_use]
    pub fn feature_path(&self) -> &str {
        &self.feature_path
    }

    /// Returns the scenario name.
    #[must_use]
    pub fn scenario_name(&self) -> &str {
        &self.scenario_name
    }

    /// Returns the one-based line number in the feature file.
    #[must_use]
    pub const fn scenario_line(&self) -> u32 {
        self.scenario_line
    }

    /// Returns the scenario tags.
    #[must_use]
    pub fn tags(&self) -> &[String] {
        &self.tags
    }
}

impl Default for ScenarioMetadata {
    fn default() -> Self {
        Self::new("<unknown>", "<unknown>", 1, Vec::new())
    }
}

/// A callable scenario runner closure owned by a harness.
///
/// # Examples
///
/// ```
/// use rstest_bdd_harness::ScenarioRunner;
///
/// let runner = ScenarioRunner::new(|value: u32| value + 1);
/// assert_eq!(runner.run(41), 42);
/// ```
pub struct ScenarioRunner<'a, C, T> {
    inner: Box<dyn FnOnce(C) -> T + 'a>,
}

impl<'a, C, T> ScenarioRunner<'a, C, T> {
    /// Wraps a closure as a scenario runner.
    #[must_use]
    pub fn new(inner: impl FnOnce(C) -> T + 'a) -> Self {
        Self {
            inner: Box::new(inner),
        }
    }

    /// Executes the wrapped closure.
    #[must_use]
    pub fn run(self, context: C) -> T {
        (self.inner)(context)
    }
}

impl<'a, T> ScenarioRunner<'a, (), T> {
    /// Wraps a zero-argument closure for harnesses that use unit context.
    #[must_use]
    pub fn new_without_context(inner: impl FnOnce() -> T + 'a) -> Self {
        Self::new(move |()| inner())
    }

    /// Executes a unit-context runner without explicitly passing `()`.
    #[must_use]
    pub fn run_without_context(self) -> T {
        self.run(())
    }
}

/// A harness execution request for one scenario.
///
/// # Examples
///
/// ```
/// use rstest_bdd_harness::{ScenarioMetadata, ScenarioRunRequest, ScenarioRunner};
///
/// let request = ScenarioRunRequest::new(
///     ScenarioMetadata::new("tests/features/auth.feature", "User signs in", 9, vec![]),
///     ScenarioRunner::new(|value: i32| value.to_string()),
/// );
/// assert_eq!(request.run(7), "7");
/// ```
pub struct ScenarioRunRequest<'a, C, T> {
    metadata: ScenarioMetadata,
    runner: ScenarioRunner<'a, C, T>,
}

impl<'a, C, T> ScenarioRunRequest<'a, C, T> {
    /// Creates a request from metadata and a runner.
    #[must_use]
    pub fn new(metadata: ScenarioMetadata, runner: ScenarioRunner<'a, C, T>) -> Self {
        Self { metadata, runner }
    }

    /// Returns immutable metadata for diagnostics or harness setup.
    #[must_use]
    pub fn metadata(&self) -> &ScenarioMetadata {
        &self.metadata
    }

    /// Consumes the request and returns metadata and runner separately.
    #[must_use]
    pub fn into_parts(self) -> (ScenarioMetadata, ScenarioRunner<'a, C, T>) {
        (self.metadata, self.runner)
    }

    /// Executes the runner with harness-provided context.
    #[must_use]
    pub fn run(self, context: C) -> T {
        self.runner.run(context)
    }
}

impl<'a, T> ScenarioRunRequest<'a, (), T> {
    /// Creates a unit-context request from metadata and a zero-argument runner.
    #[must_use]
    pub fn new_without_context(
        metadata: ScenarioMetadata,
        runner: impl FnOnce() -> T + 'a,
    ) -> Self {
        Self::new(metadata, ScenarioRunner::new_without_context(runner))
    }

    /// Executes a unit-context request without explicitly passing `()`.
    #[must_use]
    pub fn run_without_context(self) -> T {
        self.run(())
    }
}

/// Type alias for the common unit-context runner case.
pub type StdScenarioRunner<'a, T> = ScenarioRunner<'a, (), T>;

/// Type alias for the common unit-context request case.
pub type StdScenarioRunRequest<'a, T> = ScenarioRunRequest<'a, (), T>;

#[cfg(test)]
mod tests {
    //! Unit tests for scenario metadata and runner primitives.

    use super::{ScenarioMetadata, ScenarioRunRequest, ScenarioRunner};
    use std::cell::Cell;
    use std::rc::Rc;

    #[test]
    fn metadata_default_is_unknown() {
        let metadata = ScenarioMetadata::default();
        assert_eq!(metadata.feature_path(), "<unknown>");
        assert_eq!(metadata.scenario_name(), "<unknown>");
        assert_eq!(metadata.scenario_line(), 1);
        assert!(metadata.tags().is_empty());
    }

    #[test]
    fn scenario_runner_executes_closure() {
        let flag = Rc::new(Cell::new(false));
        let flag_clone = Rc::clone(&flag);
        let runner = ScenarioRunner::new_without_context(move || {
            flag_clone.set(true);
            7
        });
        assert_eq!(runner.run_without_context(), 7);
        assert!(flag.get());
    }

    #[test]
    fn scenario_runner_supports_non_static_borrows() {
        let value = 42;
        let runner = ScenarioRunner::new_without_context(|| value);
        assert_eq!(runner.run_without_context(), 42);
    }

    #[test]
    fn scenario_runner_passes_context_to_closure() {
        let runner = ScenarioRunner::new(|context: u32| context + 1);
        assert_eq!(runner.run(41), 42);
    }

    #[test]
    fn request_exposes_metadata_and_runs() {
        let request = ScenarioRunRequest::new_without_context(
            ScenarioMetadata::new(
                "tests/features/auth.feature",
                "Login succeeds",
                17,
                vec!["@smoke".to_string(), "@fast".to_string()],
            ),
            || 11,
        );
        assert_eq!(request.metadata().scenario_name(), "Login succeeds");
        assert_eq!(request.metadata().scenario_line(), 17);
        assert_eq!(request.run_without_context(), 11);
    }

    #[test]
    fn request_without_context_constructor_executes_runner() {
        let request =
            ScenarioRunRequest::new_without_context(ScenarioMetadata::default(), || 5 + 6);
        assert_eq!(request.run_without_context(), 11);
    }
}
