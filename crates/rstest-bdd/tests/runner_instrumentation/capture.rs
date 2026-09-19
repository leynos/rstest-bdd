//! Capturing `tracing` spans and events, and reading their field names back.
//!
//! Split from the parent module because the two concerns are separable and each
//! is easier to review alone: this file knows how to observe what the runner
//! emits, and the parent knows which fields D14 requires. The split is also
//! forced by `module_max_lines`, whose remedy is exactly this — the same
//! pressure that produced `runner/tests/surface/walk.rs`.
//!
//! # Why a hand-rolled subscriber
//!
//! `tracing-subscriber` is not a dependency of `crates/rstest-bdd`, and adding
//! one to read a handful of field names would be a large instrument for a small
//! obligation. The house idiom is a minimal [`Subscriber`] installed with
//! `tracing::subscriber::set_default`, which scopes it to the current thread
//! and so keeps tests independent under both `cargo test` and `cargo-nextest`;
//! `src/context/tests/warning_delivery.rs` is the precedent.
//!
//! # What this cannot see
//!
//! The subscriber records field *names*, and the levels they were emitted at,
//! not their values: a `tracing` field's type is fixed at the macro, so reading
//! one back generically means implementing a second visitor per type. So an
//! assertion like "the span carries `allow_skipped`" establishes that the field
//! is present and named as documented, not that it holds this plan's value. The
//! values are checked where they are decided — `engine/policy_tests` for the
//! policy, `runner/tests/outcome.rs` for the statuses — and the parent module
//! checks that the two halves are wired together at all.

use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
};

use tracing::{
    Event,
    Level,
    Metadata,
    Subscriber,
    field::{Field, Visit},
    span::{Attributes, Id, Record},
};

/// One captured span or event: the level it was emitted at, its name, and the
/// names of every field it carried.
///
/// Field names are collected into a `BTreeSet` so an assertion about which
/// fields are present does not depend on the order `tracing` visits them in —
/// which is not guaranteed, and which would otherwise make the parent module's
/// tests order-sensitive for no reason anyone could act on.
#[derive(Debug, Clone)]
pub(super) struct Captured {
    /// The level the span or event was emitted at.
    pub(super) level: Level,
    /// The span's name for a span; the event's metadata name for an event.
    pub(super) name: String,
    /// Every field name carried, the message field included.
    pub(super) fields: BTreeSet<String>,
}

/// Captures shared between a [`CapturingSubscriber`] and the test reading it.
pub(super) type Captures = Arc<Mutex<Vec<Captured>>>;

/// A visitor that records the *names* of the fields it is shown.
///
/// Every `record_*` method is overridden deliberately. The trait's defaults are
/// no-ops, so a field whose type had no override would be silently absent from
/// the capture, and its assertion would then fail for a reason having nothing to
/// do with the runner. The values are discarded; see the module docs.
struct FieldNames<'a> {
    /// Where each visited field's name is recorded.
    fields: &'a mut BTreeSet<String>,
}

impl Visit for FieldNames<'_> {
    fn record_debug(&mut self, field: &Field, _: &dyn std::fmt::Debug) {
        let _ = self.fields.insert(field.name().to_owned());
    }

    fn record_str(&mut self, field: &Field, _: &str) {
        let _ = self.fields.insert(field.name().to_owned());
    }

    fn record_i64(&mut self, field: &Field, _: i64) {
        let _ = self.fields.insert(field.name().to_owned());
    }

    fn record_u64(&mut self, field: &Field, _: u64) {
        let _ = self.fields.insert(field.name().to_owned());
    }

    fn record_bool(&mut self, field: &Field, _: bool) {
        let _ = self.fields.insert(field.name().to_owned());
    }
}

/// Records every span and event a level filter admits, with their field names.
struct CapturingSubscriber {
    /// The most verbose level to capture. `TRACE` admits all four D14 events.
    max_level: Level,
    /// Where captured spans and events accumulate.
    captured: Captures,
}

impl Subscriber for CapturingSubscriber {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool { *metadata.level() <= self.max_level }

    fn new_span(&self, span: &Attributes<'_>) -> Id {
        let mut fields = BTreeSet::new();
        span.record(&mut FieldNames {
            fields: &mut fields,
        });
        self.push(Captured {
            level: *span.metadata().level(),
            name: span.metadata().name().to_owned(),
            fields,
        });
        // Any id will do: nothing in these tests enters or exits a span, so the
        // id is never looked up again.
        Id::from_u64(1)
    }

    fn record(&self, _: &Id, _: &Record<'_>) {}

    fn record_follows_from(&self, _: &Id, _: &Id) {}

    fn event(&self, event: &Event<'_>) {
        let mut fields = BTreeSet::new();
        event.record(&mut FieldNames {
            fields: &mut fields,
        });
        self.push(Captured {
            level: *event.metadata().level(),
            name: event.metadata().name().to_owned(),
            fields,
        });
    }

    fn enter(&self, _: &Id) {}

    fn exit(&self, _: &Id) {}
}

impl CapturingSubscriber {
    /// Append one capture, recovering from a poisoned lock.
    ///
    /// Recovery rather than propagation: an assertion in the parent module
    /// panics while holding this lock, and the failing test's message is the
    /// useful report. A second panic about the lock would replace it with a
    /// worse one.
    fn push(&self, captured: Captured) {
        self.captured
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(captured);
    }
}

/// Install a [`CapturingSubscriber`] for this thread, admitting `max_level`.
///
/// Returns the shared capture alongside the guard that keeps the subscriber
/// active. Scoped to the thread rather than global, so tests stay independent.
pub(super) fn capture(max_level: Level) -> (Captures, tracing::subscriber::DefaultGuard) {
    let captured: Captures = Arc::new(Mutex::new(Vec::new()));
    let subscriber = CapturingSubscriber {
        max_level,
        captured: Arc::clone(&captured),
    };
    (captured, tracing::subscriber::set_default(subscriber))
}

/// The captures seen so far, cloned out from under the lock.
pub(super) fn seen(captured: &Captures) -> Vec<Captured> {
    captured
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

/// The fields on the DEBUG `scenario` span, or a panic naming what was seen.
///
/// Looking the span up by name *and* level, rather than taking the first DEBUG
/// capture, is what keeps this honest: a span the runner forgot to open leaves
/// an empty capture list, not a wrong span, and the panic then says so.
pub(super) fn scenario_span_fields(captured: &Captures) -> BTreeSet<String> {
    let all = seen(captured);
    let found = all
        .iter()
        .find(|item| item.level == Level::DEBUG && item.name == "scenario")
        .map(|item| item.fields.clone());
    let Some(fields) = found else {
        panic!("the run must open a `scenario` span at DEBUG; captured {all:?}");
    };
    fields
}

/// The first capture carrying a field of this name, or a panic naming the rest.
pub(super) fn carrying<'a>(captured: &'a [Captured], field: &str) -> &'a Captured {
    let found = captured.iter().find(|item| item.fields.contains(field));
    let Some(item) = found else {
        panic!(
            "no capture carried `{field}`; the captured field names were {:?}",
            field_names(captured),
        );
    };
    item
}

/// Every field name mentioned anywhere in a capture list, for a failure message.
fn field_names(captured: &[Captured]) -> BTreeSet<&str> {
    captured
        .iter()
        .flat_map(|item| item.fields.iter().map(String::as_str))
        .collect()
}

/// The trace-level per-step events, one per recorded invocation.
pub(super) fn per_step_events(captured: &[Captured]) -> Vec<&Captured> {
    captured
        .iter()
        .filter(|item| item.level == Level::TRACE && item.fields.contains("status"))
        .collect()
}
