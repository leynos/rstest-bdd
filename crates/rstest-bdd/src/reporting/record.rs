//! Data structures representing scenario results captured by the reporter.

/// Metadata describing a scenario to be recorded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScenarioMetadata {
    /// Path to the feature file containing the scenario.
    pub feature_path: String,
    /// Human-readable scenario name.
    pub scenario_name: String,
    /// Line number where the scenario is declared.
    pub line: u32,
    /// Tags applied to the scenario.
    pub tags: Vec<String>,
}

impl ScenarioMetadata {
    /// Create metadata from compile-time values.
    ///
    /// # Examples
    /// ```
    /// use rstest_bdd::reporting::ScenarioMetadata;
    ///
    /// let metadata = ScenarioMetadata::new(
    ///     "features/example.feature",
    ///     "example scenario",
    ///     3,
    ///     vec!["@allow_skipped".into()],
    /// );
    /// assert_eq!(metadata.line, 3);
    /// ```
    #[must_use]
    pub fn new(
        feature_path: impl Into<String>,
        scenario_name: impl Into<String>,
        line: u32,
        tags: impl Into<Vec<String>>,
    ) -> Self {
        Self {
            feature_path: feature_path.into(),
            scenario_name: scenario_name.into(),
            line,
            tags: tags.into(),
        }
    }
}

/// Outcome recorded for a single scenario execution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScenarioRecord {
    feature_path: String,
    scenario_name: String,
    line: u32,
    tags: Vec<String>,
    status: ScenarioStatus,
}

impl ScenarioRecord {
    /// Construct a new record for the provided scenario metadata.
    ///
    /// # Examples
    /// ```
    /// use rstest_bdd::reporting::{ScenarioRecord, ScenarioStatus};
    ///
    /// let record = ScenarioRecord::new(
    ///     "features/example.feature",
    ///     "example scenario",
    ///     3,
    ///     vec!["@allow_skipped".into()],
    ///     ScenarioStatus::Passed,
    /// );
    /// assert_eq!(record.feature_path(), "features/example.feature");
    /// assert_eq!(record.scenario_name(), "example scenario");
    /// assert_eq!(record.line(), 3);
    /// assert_eq!(record.tags(), ["@allow_skipped"]);
    /// assert!(matches!(record.status(), ScenarioStatus::Passed));
    /// ```
    #[must_use]
    pub fn new(
        feature_path: impl Into<String>,
        scenario_name: impl Into<String>,
        line: u32,
        tags: impl Into<Vec<String>>,
        status: ScenarioStatus,
    ) -> Self {
        Self {
            feature_path: feature_path.into(),
            scenario_name: scenario_name.into(),
            line,
            tags: tags.into(),
            status,
        }
    }

    /// Construct a record from prepared scenario metadata.
    ///
    /// # Examples
    /// ```
    /// use rstest_bdd::reporting::{ScenarioMetadata, ScenarioRecord, ScenarioStatus};
    ///
    /// let metadata = ScenarioMetadata::new("feature", "scenario", 1, Vec::new());
    /// let record = ScenarioRecord::from_metadata(metadata, ScenarioStatus::Passed);
    /// assert_eq!(record.scenario_name(), "scenario");
    /// ```
    #[must_use]
    pub fn from_metadata(metadata: ScenarioMetadata, status: ScenarioStatus) -> Self {
        Self {
            feature_path: metadata.feature_path,
            scenario_name: metadata.scenario_name,
            line: metadata.line,
            tags: metadata.tags,
            status,
        }
    }

    /// Access the recorded feature path.
    ///
    /// # Examples
    /// ```
    /// use rstest_bdd::reporting::{ScenarioRecord, ScenarioStatus};
    ///
    /// let record = ScenarioRecord::new(
    ///     "feature",
    ///     "scenario",
    ///     1,
    ///     Vec::new(),
    ///     ScenarioStatus::Passed,
    /// );
    /// assert_eq!(record.feature_path(), "feature");
    /// ```
    #[must_use]
    pub fn feature_path(&self) -> &str {
        &self.feature_path
    }

    /// Access the recorded scenario name.
    ///
    /// # Examples
    /// ```
    /// use rstest_bdd::reporting::{ScenarioRecord, ScenarioStatus};
    ///
    /// let record = ScenarioRecord::new(
    ///     "feature",
    ///     "scenario",
    ///     1,
    ///     Vec::new(),
    ///     ScenarioStatus::Passed,
    /// );
    /// assert_eq!(record.scenario_name(), "scenario");
    /// ```
    #[must_use]
    pub fn scenario_name(&self) -> &str {
        &self.scenario_name
    }

    /// Access the recorded scenario line number.
    #[must_use]
    pub fn line(&self) -> u32 {
        self.line
    }

    /// Access the recorded scenario tags.
    #[must_use]
    pub fn tags(&self) -> &[String] {
        &self.tags
    }

    /// Access the stored status value.
    ///
    /// # Examples
    /// ```
    /// use rstest_bdd::reporting::{ScenarioRecord, ScenarioStatus};
    ///
    /// let record = ScenarioRecord::new(
    ///     "feature",
    ///     "scenario",
    ///     1,
    ///     Vec::new(),
    ///     ScenarioStatus::Passed,
    /// );
    /// assert!(matches!(record.status(), ScenarioStatus::Passed));
    /// ```
    #[must_use]
    pub fn status(&self) -> &ScenarioStatus {
        &self.status
    }
}

/// Status of a scenario execution recorded by the collector.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScenarioStatus {
    /// Scenario executed every step and reached the body without errors.
    Passed,
    /// Scenario requested to skip at runtime.
    Skipped(SkippedScenario),
}

impl ScenarioStatus {
    /// Retrieve the lowercase label for the stored status.
    ///
    /// # Examples
    /// ```
    /// use rstest_bdd::reporting::{ScenarioRecord, ScenarioStatus};
    ///
    /// let passed = ScenarioRecord::new(
    ///     "feature",
    ///     "scenario",
    ///     1,
    ///     Vec::new(),
    ///     ScenarioStatus::Passed,
    /// );
    /// assert_eq!(passed.status().label(), "passed");
    /// ```
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Skipped(_) => "skipped",
        }
    }
}

/// Details captured when a scenario skips.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkippedScenario {
    message: Option<String>,
    allow_skipped: bool,
    forced_failure: bool,
}

impl SkippedScenario {
    /// Create a new skip record with the supplied metadata.
    ///
    /// # Examples
    /// ```
    /// use rstest_bdd::reporting::{ScenarioStatus, SkippedScenario, ScenarioRecord};
    ///
    /// let skipped = SkippedScenario::new(Some("pending".into()), true, false);
    /// let record = ScenarioRecord::new(
    ///     "feature",
    ///     "scenario",
    ///     1,
    ///     Vec::new(),
    ///     ScenarioStatus::Skipped(skipped.clone()),
    /// );
    /// assert!(matches!(
    ///     record.status(),
    ///     ScenarioStatus::Skipped(data) if data.message() == Some("pending")
    /// ));
    /// ```
    #[must_use]
    pub fn new(message: Option<String>, allow_skipped: bool, forced_failure: bool) -> Self {
        Self {
            message,
            allow_skipped,
            forced_failure,
        }
    }

    /// Retrieve the message provided by the skipping step, if any.
    ///
    /// # Examples
    /// ```
    /// use rstest_bdd::reporting::SkippedScenario;
    ///
    /// let skipped = SkippedScenario::new(Some("not implemented".into()), true, false);
    /// assert_eq!(skipped.message(), Some("not implemented"));
    /// ```
    #[must_use]
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    /// Whether the scenario allowed skipping without failing the run.
    ///
    /// # Examples
    /// ```
    /// use rstest_bdd::reporting::SkippedScenario;
    ///
    /// let skipped = SkippedScenario::new(None, true, false);
    /// assert!(skipped.allow_skipped());
    /// ```
    #[must_use]
    pub fn allow_skipped(&self) -> bool {
        self.allow_skipped
    }

    /// Whether a skip forced the suite to fail due to the global configuration.
    ///
    /// # Examples
    /// ```
    /// use rstest_bdd::reporting::SkippedScenario;
    ///
    /// let skipped = SkippedScenario::new(None, false, true);
    /// assert!(skipped.forced_failure());
    /// ```
    #[must_use]
    pub fn forced_failure(&self) -> bool {
        self.forced_failure
    }
}
