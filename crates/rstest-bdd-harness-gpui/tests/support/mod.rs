//! Shared helpers for the `scenario_name_in_logs` integration tests.
//!
//! Holds the snapshot redaction policy and the tracing recorder so the test
//! module itself stays focused on the behaviour under test.

use std::{
    fmt,
    sync::{Arc, Mutex},
};

use tracing::{
    Event,
    Subscriber,
    field::{Field, Visit},
};
use tracing_subscriber::{Layer, layer::Context};

/// Returns [`insta::Settings`] with redactions for nondeterministic data only.
///
/// Snapshot bodies must pin the exact feature path, scenario name, and feature
/// line so that regressions in the scenario-name diagnostic are caught.  The
/// only redactions applied here cover values that genuinely vary across runs:
/// thread IDs in panic headers, the Rust source file line and column of the
/// panic site, the `TypeId` hex emitted for opaque payloads, and the panic
/// hook's backtrace block.
///
/// A captured backtrace is environment-dependent, not a property of the
/// diagnostic under test: its frames vary with `RUST_BACKTRACE`, with the
/// debuginfo an instrumented coverage build retains, and with inlining. The
/// panic hook prints a backtrace where it would otherwise print the "run with
/// `RUST_BACKTRACE=1`" note, and prints that note only once per process, so
/// neither form is stable across lanes. Stripping both leaves the snapshots
/// independent of backtrace presence while asserting the diagnostic verbatim.
pub fn configured_snapshot_settings() -> insta::Settings {
    let mut settings = insta::Settings::clone_current();
    for (pattern, replacement) in &[
        (
            concat!(
                r"(?m)^stack backtrace:\n",
                r"(?:[ \t]+\d+: .*\n|[ \t]+at .*\n)*",
                r"(?:[ \t]*note: Some details are omitted.*\n)?",
            ),
            "",
        ),
        (r"(?m)^note: run with `RUST_BACKTRACE=1`[^\n]*\n", ""),
        // Anchor the thread-id redaction to the Rust panic-header form
        // `thread '<name>' (<id>) panicked at ...` so unrelated
        // parenthesized integers in panic payloads (e.g. struct variants)
        // are preserved in snapshots.
        (r"(thread '[^']*') \(\d+\)", "$1 ([TID])"),
        (r"\.rs:\d+:\d+", ".rs:[LINE]:[COL]"),
        (r"TypeId\(0x[0-9a-f]+\)", "TypeId([TYPEID])"),
    ] {
        settings.add_filter(pattern, *replacement);
    }
    settings
}

pub struct RecordingLayer {
    pub events: Arc<Mutex<Vec<String>>>,
}

impl<S> Layer<S> for RecordingLayer
where
    S: Subscriber,
{
    /// Visits every tracing event, serializes its fields, and appends the
    /// result to the shared event buffer for later inspection.
    fn on_event(&self, event: &Event<'_>, _context: Context<'_, S>) {
        let mut visitor = EventVisitor::default();
        event.record(&mut visitor);
        let mut events = match self.events.lock() {
            Ok(events) => events,
            Err(error) => panic!("captured tracing events should not be poisoned: {error}"),
        };
        events.push(visitor.fields.join(" "));
    }
}

#[derive(Default)]
struct EventVisitor {
    fields: Vec<String>,
}

impl Visit for EventVisitor {
    /// Records the debug representation of a tracing field into the
    /// accumulated field list.
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.fields.push(format!("{}={value:?}", field.name()));
    }
}
