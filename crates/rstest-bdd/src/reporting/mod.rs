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
use std::sync::{Mutex, MutexGuard, OnceLock};

#[cfg(feature = "diagnostics")]
use std::sync::atomic::{AtomicBool, Ordering};

mod record;
pub use record::{ScenarioMetadata, ScenarioRecord, ScenarioStatus, SkippedScenario};

/// JSON report writer for scenario outcomes.
#[cfg(feature = "diagnostics")]
pub mod json;
/// JUnit XML writer for scenario outcomes.
#[cfg(feature = "diagnostics")]
pub mod junit;

#[cfg(feature = "diagnostics")]
static RUN_DUMP_SEEDS: OnceLock<AtomicBool> = OnceLock::new();

#[cfg(feature = "diagnostics")]
fn dump_seeds_guard() -> &'static AtomicBool {
    RUN_DUMP_SEEDS.get_or_init(|| AtomicBool::new(false))
}

#[cfg(feature = "diagnostics")]
fn reset_dump_seeds_guard() {
    if let Some(flag) = RUN_DUMP_SEEDS.get() {
        flag.store(false, Ordering::SeqCst);
    }
}

/// Thread-safe store containing scenario records gathered during a test run.
static REPORTS: OnceLock<Mutex<Vec<ScenarioRecord>>> = OnceLock::new();

fn reports_mutex() -> &'static Mutex<Vec<ScenarioRecord>> {
    REPORTS.get_or_init(|| Mutex::new(Vec::new()))
}

fn lock_reports() -> MutexGuard<'static, Vec<ScenarioRecord>> {
    // Recover from poisoned locks so diagnostics can still flush any
    // accumulated records when a prior test panicked whilst holding the
    // mutex. The collector only serves tests and short-lived binaries, so
    // preserving the captured outcomes aids troubleshooting more than
    // propagating the panic context.
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
/// reporting collector before the registry is serialized.
///
/// # Examples
/// ```ignore
/// rstest_bdd::reporting::run_dump_seeds();
/// ```
#[cfg(feature = "diagnostics")]
pub fn run_dump_seeds() {
    if dump_seeds_guard().swap(true, Ordering::SeqCst) {
        return;
    }
    for seed in inventory::iter::<DumpSeed> {
        seed.run();
    }
}

/// Record a scenario outcome in the shared collector.
///
/// # Examples
/// ```
/// use rstest_bdd::reporting::{
///     drain, record, snapshot, ScenarioMetadata, ScenarioRecord, ScenarioStatus,
/// };
///
/// let metadata = ScenarioMetadata::new("feature", "scenario", 1, Vec::new());
/// record(ScenarioRecord::from_metadata(metadata, ScenarioStatus::Passed));
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
/// use rstest_bdd::reporting::{record, snapshot, ScenarioMetadata, ScenarioRecord, ScenarioStatus};
///
/// let metadata = ScenarioMetadata::new("feature", "scenario", 1, Vec::new());
/// record(ScenarioRecord::from_metadata(metadata, ScenarioStatus::Passed));
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
/// use rstest_bdd::reporting::{drain, record, snapshot, ScenarioMetadata, ScenarioRecord, ScenarioStatus};
///
/// let metadata = ScenarioMetadata::new("feature", "scenario", 1, Vec::new());
/// record(ScenarioRecord::from_metadata(metadata, ScenarioStatus::Passed));
/// let drained = drain();
/// assert!(snapshot().is_empty());
/// assert_eq!(drained.len(), 1);
/// ```
#[must_use]
pub fn drain() -> Vec<ScenarioRecord> {
    let drained = lock_reports().drain(..).collect();
    #[cfg(feature = "diagnostics")]
    reset_dump_seeds_guard();
    drained
}

#[cfg(test)]
mod tests;
