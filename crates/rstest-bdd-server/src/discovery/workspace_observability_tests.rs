//! Deterministic telemetry coverage for public feature-file discovery.

use std::{
    fmt,
    io,
    path::PathBuf,
    sync::{Arc, Mutex, PoisonError},
};

use metrics::{
    Counter,
    CounterFn,
    Gauge,
    Histogram,
    HistogramFn,
    Key,
    KeyName,
    Metadata,
    Recorder,
    SharedString,
    Unit,
    with_local_recorder,
};
use tempfile::TempDir;
use tracing::{
    Event,
    Level,
    Subscriber,
    field::{Field, Visit},
};
use tracing_subscriber::{Layer, Registry, layer::Context, prelude::*};

use super::*;

/// Records feature-file discovery metrics for assertions without global state.
#[derive(Default)]
struct DiscoveryRecorder {
    counters: Arc<Mutex<Vec<RecordedCounter>>>,
    durations: Arc<Mutex<Vec<f64>>>,
}

impl DiscoveryRecorder {
    /// Returns the total recorded value for one bounded discovery outcome.
    fn outcome_count(&self, outcome: &str) -> u64 {
        self.counters
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .filter(|counter| {
                counter.name == FEATURE_FILE_DISCOVERY_COUNTER && counter.outcome == outcome
            })
            .map(|counter| counter.value)
            .sum()
    }

    /// Returns the count of duration samples emitted by discovery calls.
    fn duration_count(&self) -> usize {
        self.durations
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }
}

/// Stores one bounded counter registration and its accumulated value.
struct RecordedCounter {
    name: String,
    outcome: String,
    value: u64,
}

/// Updates the local counter backing one feature-discovery outcome.
struct DiscoveryCounter {
    counters: Arc<Mutex<Vec<RecordedCounter>>>,
    name: String,
    outcome: String,
}

impl CounterFn for DiscoveryCounter {
    fn increment(&self, value: u64) {
        let mut counters = self.counters.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(counter) = counters
            .iter_mut()
            .find(|counter| counter.name == self.name && counter.outcome == self.outcome)
        {
            counter.value += value;
        }
    }

    fn absolute(&self, value: u64) {
        let mut counters = self.counters.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(counter) = counters
            .iter_mut()
            .find(|counter| counter.name == self.name && counter.outcome == self.outcome)
        {
            counter.value = counter.value.max(value);
        }
    }
}

/// Records one feature-discovery duration sample in the local recorder.
struct DiscoveryHistogram {
    durations: Arc<Mutex<Vec<f64>>>,
}

impl HistogramFn for DiscoveryHistogram {
    fn record(&self, value: f64) {
        self.durations
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(value);
    }
}

impl Recorder for DiscoveryRecorder {
    fn describe_counter(&self, _: KeyName, _: Option<Unit>, _: SharedString) {}

    fn describe_gauge(&self, _: KeyName, _: Option<Unit>, _: SharedString) {}

    fn describe_histogram(&self, _: KeyName, _: Option<Unit>, _: SharedString) {}

    fn register_counter(&self, key: &Key, _: &Metadata<'_>) -> Counter {
        if key.name() != FEATURE_FILE_DISCOVERY_COUNTER {
            return Counter::noop();
        }

        let outcome = key
            .labels()
            .find(|label| label.key() == "outcome")
            .map(|label| label.value().to_owned())
            .unwrap_or_default();
        let name = key.name().to_owned();
        self.counters
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(RecordedCounter {
                name: name.clone(),
                outcome: outcome.clone(),
                value: 0,
            });
        Counter::from_arc(Arc::new(DiscoveryCounter {
            counters: Arc::clone(&self.counters),
            name,
            outcome,
        }))
    }

    fn register_gauge(&self, _: &Key, _: &Metadata<'_>) -> Gauge { Gauge::noop() }

    fn register_histogram(&self, key: &Key, _: &Metadata<'_>) -> Histogram {
        if key.name() != FEATURE_FILE_DISCOVERY_DURATION {
            return Histogram::noop();
        }

        Histogram::from_arc(Arc::new(DiscoveryHistogram {
            durations: Arc::clone(&self.durations),
        }))
    }
}

/// Captures warning-event fields emitted by feature-file discovery.
struct WarningRecordingLayer {
    events: Arc<Mutex<Vec<String>>>,
}

impl<S> Layer<S> for WarningRecordingLayer
where
    S: Subscriber,
{
    /// Serializes warning fields into the shared buffer for later assertions.
    fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
        if *event.metadata().level() != Level::WARN {
            return;
        }

        let mut visitor = EventVisitor::default();
        event.record(&mut visitor);
        self.events
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(visitor.fields.join(" "));
    }
}

/// Collects the debug representations of fields attached to one tracing event.
#[derive(Default)]
struct EventVisitor {
    fields: Vec<String>,
}

impl Visit for EventVisitor {
    /// Stores a field name and its debug representation for assertion output.
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.fields.push(format!("{}={value:?}", field.name()));
    }
}

/// Runs an operation with local metrics and warning-event recorders installed.
fn with_observability_recording<T>(
    recorder: &DiscoveryRecorder,
    events: Arc<Mutex<Vec<String>>>,
    operation: impl FnOnce() -> T,
) -> T {
    let subscriber = Registry::default().with(WarningRecordingLayer { events });

    with_local_recorder(recorder, || {
        tracing::subscriber::with_default(subscriber, operation)
    })
}

/// Creates a missing path that deterministically produces `io::ErrorKind::NotFound`.
fn create_missing_workspace_path() -> PathBuf {
    let temporary_directory = match TempDir::new() {
        Ok(temporary_directory) => temporary_directory,
        Err(error) => panic!("failed to create temp dir: {error}"),
    };
    let missing_path = temporary_directory.path().to_path_buf();
    if let Err(error) = temporary_directory.close() {
        panic!("failed to remove temp dir: {error}");
    }

    missing_path
}

/// Records one successful counter and duration without emitting a warning.
#[test]
fn records_successful_feature_discovery_observability() {
    let workspace = TempDir::new().expect("workspace directory should be created");
    let recorder = DiscoveryRecorder::default();
    let events = Arc::new(Mutex::new(Vec::new()));

    let result = with_observability_recording(&recorder, Arc::clone(&events), || {
        find_feature_files(workspace.path())
    });

    let features = result.expect("feature discovery should succeed");
    assert!(features.is_empty());
    assert_eq!(recorder.outcome_count("success"), 1);
    assert_eq!(recorder.duration_count(), 1);
    assert!(
        events
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_empty(),
        "successful discovery must not emit a warning",
    );
}

/// Records an I/O failure and logs its workspace root and underlying error.
#[test]
fn records_failed_feature_discovery_observability() {
    let missing_path = create_missing_workspace_path();
    let workspace_root = missing_path.display().to_string();
    let recorder = DiscoveryRecorder::default();
    let events = Arc::new(Mutex::new(Vec::new()));

    let result = with_observability_recording(&recorder, Arc::clone(&events), || {
        find_feature_files(&missing_path)
    });

    let Err(ServerError::Io(error)) = result else {
        panic!("missing workspace should return an I/O error");
    };
    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    assert_eq!(recorder.outcome_count("io-failure"), 1);
    assert_eq!(recorder.duration_count(), 1);

    let events = events.lock().unwrap_or_else(PoisonError::into_inner);
    let [event] = events.as_slice() else {
        panic!("expected one feature discovery warning, got: {events:?}");
    };
    assert!(event.contains("feature file discovery failed"));
    assert!(event.contains("operation=\"feature_file_discovery\""));
    assert!(event.contains("workspace_root="));
    assert!(event.contains(&workspace_root));
    assert!(event.contains("error="));
}
