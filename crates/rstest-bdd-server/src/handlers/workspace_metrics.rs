//! Bounded metrics for asynchronous workspace preparation and deferred saves.
//!
//! Both lifecycle and text-document handlers use this module. Labels describe
//! fixed operations and outcomes only; client paths and source text never form
//! part of a metric key.

#[cfg(test)]
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

#[cfg(test)]
use metrics::{
    Counter,
    CounterFn,
    Gauge,
    GaugeFn,
    Histogram,
    HistogramFn,
    Key,
    KeyName,
    Metadata,
    Recorder,
    SharedString,
    Unit,
};
use metrics::{counter, describe_counter, describe_gauge, describe_histogram, gauge, histogram};

const WORKSPACE_COUNTER: &str = "rstest_bdd_server_workspace_preparation_total";
const DEFERRED_SAVE_GAUGE: &str = "rstest_bdd_server_deferred_document_saves";
const WORKSPACE_DURATION: &str = "rstest_bdd_server_workspace_preparation_duration_seconds";

/// Record a fixed workspace preparation or deferred-save outcome.
pub(super) fn record_workspace_outcome(operation: &'static str, outcome: &'static str) {
    describe_counter!(
        WORKSPACE_COUNTER,
        "Workspace preparation and deferred-save outcomes, labelled by operation and outcome"
    );
    counter!(WORKSPACE_COUNTER, "operation" => operation, "outcome" => outcome).increment(1);
}

/// Record the current count of did-save notifications awaiting workspace readiness.
pub(super) fn record_deferred_save_depth(depth: usize) {
    describe_gauge!(
        DEFERRED_SAVE_GAUGE,
        "Did-save notifications awaiting workspace preparation"
    );
    let bounded_depth = u8::try_from(depth).unwrap_or(u8::MAX);
    gauge!(DEFERRED_SAVE_GAUGE).set(f64::from(bounded_depth));
}

/// Record elapsed blocking workspace preparation time.
pub(super) fn record_workspace_preparation_duration(duration: Duration) {
    describe_histogram!(
        WORKSPACE_DURATION,
        "Elapsed blocking workspace preparation time in seconds"
    );
    histogram!(WORKSPACE_DURATION).record(duration.as_secs_f64());
}

#[cfg(test)]
/// A recorder for verifying bounded workspace metrics in handler tests.
#[derive(Default)]
pub(crate) struct WorkspaceRecorder {
    counters: Arc<Mutex<Vec<RecordedCounter>>>,
    gauges: Arc<Mutex<Vec<(String, f64)>>>,
    histograms: Arc<Mutex<Vec<(String, f64)>>>,
}

#[cfg(test)]
impl WorkspaceRecorder {
    /// Return the recorded count for one fixed operation/outcome pair.
    pub(crate) fn workspace_outcome_count(&self, operation: &str, outcome: &str) -> u64 {
        self.counters
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .filter(|counter| {
                counter.name == WORKSPACE_COUNTER
                    && counter.labels
                        == [
                            ("operation".to_owned(), operation.to_owned()),
                            ("outcome".to_owned(), outcome.to_owned()),
                        ]
            })
            .map(|counter| counter.count)
            .sum()
    }

    /// Return the latest recorded deferred-save queue depth.
    pub(crate) fn deferred_save_depth(&self) -> Option<f64> {
        self.gauges
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .rev()
            .find(|(name, _)| name == DEFERRED_SAVE_GAUGE)
            .map(|(_, depth)| *depth)
    }
}

#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq)]
struct RecordedCounter {
    name: String,
    labels: Vec<(String, String)>,
    count: u64,
}

#[cfg(test)]
struct CounterHandle {
    counters: Arc<Mutex<Vec<RecordedCounter>>>,
    name: String,
    labels: Vec<(String, String)>,
}

#[cfg(test)]
struct GaugeHandle {
    gauges: Arc<Mutex<Vec<(String, f64)>>>,
    name: String,
}

#[cfg(test)]
struct HistogramHandle {
    histograms: Arc<Mutex<Vec<(String, f64)>>>,
    name: String,
}

#[cfg(test)]
impl CounterFn for CounterHandle {
    fn increment(&self, value: u64) {
        let mut counters = self.counters.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(counter) = counters
            .iter_mut()
            .find(|counter| counter.name == self.name && counter.labels == self.labels)
        {
            counter.count += value;
        }
    }

    fn absolute(&self, value: u64) {
        let mut counters = self.counters.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(counter) = counters
            .iter_mut()
            .find(|counter| counter.name == self.name && counter.labels == self.labels)
        {
            counter.count = counter.count.max(value);
        }
    }
}

#[cfg(test)]
impl GaugeFn for GaugeHandle {
    fn increment(&self, _: f64) {}

    fn decrement(&self, _: f64) {}

    fn set(&self, value: f64) {
        let mut gauges = self.gauges.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some((_, gauge)) = gauges.iter_mut().find(|(name, _)| name == &self.name) {
            *gauge = value;
        }
    }
}

#[cfg(test)]
impl HistogramFn for HistogramHandle {
    fn record(&self, value: f64) {
        self.histograms
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((self.name.clone(), value));
    }
}

#[cfg(test)]
impl Recorder for WorkspaceRecorder {
    fn describe_counter(&self, _: KeyName, _: Option<Unit>, _: SharedString) {}

    fn describe_gauge(&self, _: KeyName, _: Option<Unit>, _: SharedString) {}

    fn describe_histogram(&self, _: KeyName, _: Option<Unit>, _: SharedString) {}

    fn register_counter(&self, key: &Key, _: &Metadata<'_>) -> Counter {
        let mut labels: Vec<_> = key
            .labels()
            .map(|label| (label.key().to_owned(), label.value().to_owned()))
            .collect();
        labels.sort_unstable();
        let mut counters = self.counters.lock().unwrap_or_else(PoisonError::into_inner);
        counters.push(RecordedCounter {
            name: key.name().to_owned(),
            labels: labels.clone(),
            count: 0,
        });
        Counter::from_arc(Arc::new(CounterHandle {
            counters: Arc::clone(&self.counters),
            name: key.name().to_owned(),
            labels,
        }))
    }

    fn register_gauge(&self, key: &Key, _: &Metadata<'_>) -> Gauge {
        let name = key.name().to_owned();
        self.gauges
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((name, 0.0));
        Gauge::from_arc(Arc::new(GaugeHandle {
            gauges: Arc::clone(&self.gauges),
            name: key.name().to_owned(),
        }))
    }

    fn register_histogram(&self, key: &Key, _: &Metadata<'_>) -> Histogram {
        let name = key.name().to_owned();
        Histogram::from_arc(Arc::new(HistogramHandle {
            histograms: Arc::clone(&self.histograms),
            name,
        }))
    }
}

#[cfg(test)]
mod tests {
    //! Recorder-backed tests for bounded workspace metrics.

    use metrics::with_local_recorder;

    use super::*;

    #[test]
    fn records_bounded_workspace_metric_keys_and_values() {
        let recorder = WorkspaceRecorder::default();

        with_local_recorder(&recorder, || {
            record_workspace_outcome("workspace-preparation", "success");
            record_deferred_save_depth(2);
            record_workspace_preparation_duration(Duration::from_millis(10));
        });

        let counters = recorder
            .counters
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let [counter] = counters.as_slice() else {
            panic!("expected one workspace counter");
        };
        assert_eq!(counter.name, WORKSPACE_COUNTER);
        assert_eq!(counter.count, 1);
        assert_eq!(
            counter.labels,
            vec![
                ("operation".to_owned(), "workspace-preparation".to_owned()),
                ("outcome".to_owned(), "success".to_owned()),
            ]
        );
        assert_eq!(
            recorder
                .gauges
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .as_slice(),
            [(DEFERRED_SAVE_GAUGE.to_owned(), 2.0)]
        );
        assert_eq!(
            recorder
                .histograms
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .as_slice(),
            [(WORKSPACE_DURATION.to_owned(), 0.01)]
        );
    }
}
