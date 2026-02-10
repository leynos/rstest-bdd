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
/// let runner = ScenarioRunner::new(|| 41 + 1);
/// assert_eq!(runner.run(), 42);
/// ```
pub struct ScenarioRunner<'a, T> {
    inner: Box<dyn FnOnce() -> T + 'a>,
}

impl<'a, T> ScenarioRunner<'a, T> {
    /// Wraps a closure as a scenario runner.
    #[must_use]
    pub fn new(inner: impl FnOnce() -> T + 'a) -> Self {
        Self {
            inner: Box::new(inner),
        }
    }

    /// Executes the wrapped closure.
    #[must_use]
    pub fn run(self) -> T {
        (self.inner)()
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
///     ScenarioRunner::new(|| "ok"),
/// );
/// assert_eq!(request.run(), "ok");
/// ```
pub struct ScenarioRunRequest<'a, T> {
    metadata: ScenarioMetadata,
    runner: ScenarioRunner<'a, T>,
}

impl<'a, T> ScenarioRunRequest<'a, T> {
    /// Creates a request from metadata and a runner.
    #[must_use]
    pub fn new(metadata: ScenarioMetadata, runner: ScenarioRunner<'a, T>) -> Self {
        Self { metadata, runner }
    }

    /// Returns immutable metadata for diagnostics or harness setup.
    #[must_use]
    pub fn metadata(&self) -> &ScenarioMetadata {
        &self.metadata
    }

    /// Consumes the request and returns metadata and runner separately.
    #[must_use]
    pub fn into_parts(self) -> (ScenarioMetadata, ScenarioRunner<'a, T>) {
        (self.metadata, self.runner)
    }

    /// Executes the runner directly.
    #[must_use]
    pub fn run(self) -> T {
        self.runner.run()
    }
}

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
        let runner = ScenarioRunner::new(move || {
            flag_clone.set(true);
            7
        });
        assert_eq!(runner.run(), 7);
        assert!(flag.get());
    }

    #[test]
    fn scenario_runner_supports_non_static_borrows() {
        let value = 42;
        let runner = ScenarioRunner::new(|| value);
        assert_eq!(runner.run(), 42);
    }

    #[test]
    fn request_exposes_metadata_and_runs() {
        let request = ScenarioRunRequest::new(
            ScenarioMetadata::new(
                "tests/features/auth.feature",
                "Login succeeds",
                17,
                vec!["@smoke".to_string(), "@fast".to_string()],
            ),
            ScenarioRunner::new(|| 11),
        );
        assert_eq!(request.metadata().scenario_name(), "Login succeeds");
        assert_eq!(request.metadata().scenario_line(), 17);
        assert_eq!(request.run(), 11);
    }
}
