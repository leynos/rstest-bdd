//! Scenario reporting collector.
//!
//! The module stores the outcome of each executed scenario in a global,
//! thread-safe collector. Reporters can read the collected entries to render
//! summaries in alternative formats without depending on the macro-generated
//! tests directly.
//!
//! # Concurrency
//!
//! The collector is intentionally global so that generated tests and external
//! binaries can publish their outcomes without plumbing additional context.
//! Behavioural tests that assert on the stored records must execute
//! serially (for example via [`serial_test::serial`]) to avoid cross-test
//! contamination. The API does not reset records automatically; callers
//! remain responsible for draining the collector between assertions.

use std::sync::{Mutex, MutexGuard, Once, OnceLock};

/// JSON report writer for scenario outcomes.
#[cfg(feature = "diagnostics")]
pub mod json;
/// JUnit XML writer for scenario outcomes.
#[cfg(feature = "diagnostics")]
pub mod junit;

#[cfg(feature = "diagnostics")]
static RUN_DUMP_SEEDS: Once = Once::new();

/// Thread-safe store containing scenario records gathered during a test run.
static REPORTS: OnceLock<Mutex<Vec<ScenarioRecord>>> = OnceLock::new();

fn reports_mutex() -> &'static Mutex<Vec<ScenarioRecord>> {
    REPORTS.get_or_init(|| Mutex::new(Vec::new()))
}

fn lock_reports() -> MutexGuard<'static, Vec<ScenarioRecord>> {
    match reports_mutex().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// Seed executed before emitting diagnostic step dumps.
#[cfg(feature = "diagnostics")]
#[derive(Copy, Clone)]
pub struct DumpSeed {
    callback: fn(),
}

#[cfg(feature = "diagnostics")]
impl DumpSeed {
    /// Construct a seed that will run before dumping diagnostic steps.
    ///
    /// # Examples
    /// ```ignore
    /// use rstest_bdd::reporting::DumpSeed;
    ///
    /// fn seed() {
    ///     // Record scenario metadata for diagnostics output.
    /// }
    ///
    /// inventory::submit! {
    ///     DumpSeed::new(seed)
    /// }
    /// ```
    #[must_use]
    pub const fn new(callback: fn()) -> Self {
        Self { callback }
    }

    fn run(self) {
        (self.callback)();
    }
}

#[cfg(feature = "diagnostics")]
inventory::collect!(DumpSeed);

/// Execute all registered dump seeds once per process.
///
/// Registered seeds can be used by diagnostic fixtures to populate the
/// reporting collector before the registry is serialised.
///
/// # Examples
/// ```ignore
/// rstest_bdd::reporting::run_dump_seeds();
/// ```
#[cfg(feature = "diagnostics")]
pub fn run_dump_seeds() {
    RUN_DUMP_SEEDS.call_once(|| {
        for seed in inventory::iter::<DumpSeed> {
            seed.run();
        }
    });
}

/// Outcome recorded for a single scenario execution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScenarioRecord {
    feature_path: String,
    scenario_name: String,
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
    ///     ScenarioStatus::Passed,
    /// );
    /// assert_eq!(record.feature_path(), "features/example.feature");
    /// assert_eq!(record.scenario_name(), "example scenario");
    /// assert!(matches!(record.status(), ScenarioStatus::Passed));
    /// ```
    #[must_use]
    pub fn new(
        feature_path: impl Into<String>,
        scenario_name: impl Into<String>,
        status: ScenarioStatus,
    ) -> Self {
        Self {
            feature_path: feature_path.into(),
            scenario_name: scenario_name.into(),
            status,
        }
    }

    /// Access the recorded feature path.
    ///
    /// # Examples
    /// ```
    /// use rstest_bdd::reporting::{ScenarioRecord, ScenarioStatus};
    ///
    /// let record = ScenarioRecord::new("feature", "scenario", ScenarioStatus::Passed);
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
    /// let record = ScenarioRecord::new("feature", "scenario", ScenarioStatus::Passed);
    /// assert_eq!(record.scenario_name(), "scenario");
    /// ```
    #[must_use]
    pub fn scenario_name(&self) -> &str {
        &self.scenario_name
    }

    /// Access the stored status value.
    ///
    /// # Examples
    /// ```
    /// use rstest_bdd::reporting::{ScenarioRecord, ScenarioStatus};
    ///
    /// let record = ScenarioRecord::new("feature", "scenario", ScenarioStatus::Passed);
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
    /// let passed = ScenarioRecord::new("feature", "scenario", ScenarioStatus::Passed);
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
    ///     "feature", "scenario", ScenarioStatus::Skipped(skipped.clone()),
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

/// Record a scenario outcome in the shared collector.
///
/// # Examples
/// ```
/// use rstest_bdd::reporting::{record, drain, snapshot, ScenarioRecord, ScenarioStatus};
///
/// record(ScenarioRecord::new("feature", "scenario", ScenarioStatus::Passed));
/// let records = drain();
/// assert_eq!(records.len(), 1);
/// ```
pub fn record(record: ScenarioRecord) {
    lock_reports().push(record);
}

/// Retrieve a snapshot of the recorded scenarios without clearing them.
///
/// # Examples
/// ```
/// use rstest_bdd::reporting::{record, snapshot, ScenarioRecord, ScenarioStatus};
///
/// record(ScenarioRecord::new("feature", "scenario", ScenarioStatus::Passed));
/// let records = snapshot();
/// assert_eq!(records[0].scenario_name(), "scenario");
/// ```
#[must_use]
pub fn snapshot() -> Vec<ScenarioRecord> {
    lock_reports().clone()
}

/// Remove and return all recorded scenario outcomes.
///
/// # Examples
/// ```
/// use rstest_bdd::reporting::{record, drain, ScenarioRecord, ScenarioStatus};
///
/// record(ScenarioRecord::new("feature", "scenario", ScenarioStatus::Passed));
/// let drained = drain();
/// assert!(snapshot().is_empty());
/// assert_eq!(drained.len(), 1);
/// ```
#[must_use]
pub fn drain() -> Vec<ScenarioRecord> {
    lock_reports().drain(..).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn drain_clears_records() {
        let _ = drain();
        record(ScenarioRecord::new(
            "feature",
            "scenario",
            ScenarioStatus::Passed,
        ));
        assert_eq!(snapshot().len(), 1);
        let drained = drain();
        assert_eq!(drained.len(), 1);
        assert!(snapshot().is_empty());
    }

    #[test]
    #[serial]
    fn skipped_records_store_metadata() {
        let _ = drain();
        let details = SkippedScenario::new(Some("pending".into()), true, false);
        record(ScenarioRecord::new(
            "feature",
            "scenario",
            ScenarioStatus::Skipped(details.clone()),
        ));
        let records = drain();
        assert_eq!(records.len(), 1);
        let Some(record) = records.first() else {
            panic!("collector should retain the recorded skip");
        };
        match record.status() {
            ScenarioStatus::Skipped(stored) => {
                assert_eq!(stored.message(), Some("pending"));
                assert!(stored.allow_skipped());
                assert!(!stored.forced_failure());
            }
            ScenarioStatus::Passed => panic!("expected skipped record"),
        }
    }
}
