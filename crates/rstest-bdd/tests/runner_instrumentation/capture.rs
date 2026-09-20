//! Capturing `tracing` spans and events, and reading their field values back.
//!
//! Split from the parent module because the two concerns are separable and each
//! is easier to review alone: this file knows how to observe what the runner
//! emits, and the parent knows which fields D14 requires and what they must
//! hold. The split is also forced by `module_max_lines`, whose remedy is
//! exactly this — the same pressure that produced `runner/tests/surface/walk.rs`.
//!
//! # Why a hand-rolled subscriber
//!
//! `tracing-subscriber` is not a dependency of `crates/rstest-bdd`, and adding
//! one to read a handful of field values would be a large instrument for a small
//! obligation. The house idiom is a minimal [`Subscriber`] installed with
//! `tracing::subscriber::set_default`, which scopes it to the current thread
//! and so keeps tests independent under both `cargo test` and `cargo-nextest`;
//! `src/context/tests/warning_delivery.rs` is the precedent.
//!
//! # What this can and cannot see
//!
//! The subscriber records each field's *rendered* value alongside its name, so
//! the parent module can assert that the runner logged `index = 0` rather than
//! merely that it logged something called `index`. That distinction is not
//! cosmetic: every field assertion in the parent would still hold against a
//! runner that emitted `index + 1`, or that transposed two fields' values,
//! because presence is all a name-only capture can witness.
//!
//! What it cannot see is the value's *type*. Everything is rendered to a
//! `String` on the way in, because that is what a `Visit` hands over — a
//! rendered form, not a typed one. So an assertion here compares text, and a
//! change to a field's rendering would show up as a changed assertion rather
//! than as a type error. Two consequences are worth stating: a `str` field is
//! recorded unquoted and everything else through its `Debug` form, so an
//! assertion on an `Option<u32>` reads `Some(42)`; and the capture cannot
//! distinguish a field that was emitted as `0u32` from one emitted as `0u64`.
//! Neither matters for D14, which specifies what a field must hold rather than
//! how it is typed, and the field's name is what a subscriber filters on.

use std::{
    collections::BTreeMap,
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
/// rendered value of every field it carried.
///
/// Values are collected into a `BTreeMap` so an assertion about them does not
/// depend on the order `tracing` visits them in — which is not guaranteed, and
/// which would otherwise make the parent module's tests order-sensitive for no
/// reason anyone could act on. The map's key set is the set of field names, so
/// a map rather than a set-plus-map: the two would be the same information
/// stored twice, and could drift apart.
#[derive(Debug, Clone)]
pub(super) struct Captured {
    /// The level the span or event was emitted at.
    pub(super) level: Level,
    /// The span's name for a span; the event's metadata name for an event.
    pub(super) name: String,
    /// Every field carried, the message field included, by rendered value.
    pub(super) values: BTreeMap<String, String>,
}

impl Captured {
    /// Whether this capture carried a field of this name.
    pub(super) fn carries(&self, field: &str) -> bool { self.values.contains_key(field) }
}

/// The rendered value of one field, or a panic naming every field that was.
///
/// The panic prints the whole map rather than only the missing name, because
/// the useful question when an assertion about `location` fails is what the
/// runner emitted instead — and that is not answerable from the name alone.
/// `#[track_caller]` so the reported location is the assertion that asked, not
/// this helper.
#[track_caller]
pub(super) fn value<'a>(values: &'a BTreeMap<String, String>, field: &str) -> &'a str {
    let Some(found) = values.get(field) else {
        panic!("no `{field}` field was recorded; the recorded fields were {values:?}");
    };
    found.as_str()
}

/// Captures shared between a [`CapturingSubscriber`] and the test reading it.
pub(super) type Captures = Arc<Mutex<Vec<Captured>>>;

/// A visitor that records the *value* of each field it is shown, rendered.
///
/// Only `record_str`, `record_i64`, `record_u64`, and `record_bool` are
/// overridden; every other `record_*` method falls through to
/// [`record_debug`](Visit::record_debug) below, which `note`s the `Debug`
/// rendering. The overrides exist because the trait's defaults also forward to
/// `record_debug`, so without them a `&str` field would read back with its
/// quotes still attached — see `record_str`.
struct FieldValues<'a> {
    /// Where each visited field's rendered value is recorded.
    values: &'a mut BTreeMap<String, String>,
}

impl FieldValues<'_> {
    /// Record one field's rendered value under its name.
    fn note(&mut self, field: &Field, rendered: String) {
        let _ = self.values.insert(field.name().to_owned(), rendered);
    }
}

impl Visit for FieldValues<'_> {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.note(field, format!("{value:?}"));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        // Unquoted, unlike the `Debug` form above: a path or a message read back
        // with its quotes still attached would make every assertion about it
        // read as an escaped string literal rather than as the text a
        // subscriber would print.
        self.note(field, value.to_owned());
    }

    fn record_i64(&mut self, field: &Field, value: i64) { self.note(field, value.to_string()); }

    fn record_u64(&mut self, field: &Field, value: u64) { self.note(field, value.to_string()); }

    fn record_bool(&mut self, field: &Field, value: bool) { self.note(field, value.to_string()); }
}

/// Records every span and event a level filter admits, with their field values.
struct CapturingSubscriber {
    /// The most verbose level to capture. `TRACE` admits all four D14 events.
    max_level: Level,
    /// Where captured spans and events accumulate.
    captured: Captures,
}

impl Subscriber for CapturingSubscriber {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool { *metadata.level() <= self.max_level }

    fn new_span(&self, span: &Attributes<'_>) -> Id {
        let mut values = BTreeMap::new();
        span.record(&mut FieldValues {
            values: &mut values,
        });
        self.push(Captured {
            level: *span.metadata().level(),
            name: span.metadata().name().to_owned(),
            values,
        });
        // Any id will do: nothing in these tests enters or exits a span, so the
        // id is never looked up again.
        Id::from_u64(1)
    }

    fn record(&self, _: &Id, _: &Record<'_>) {}

    fn record_follows_from(&self, _: &Id, _: &Id) {}

    fn event(&self, event: &Event<'_>) {
        let mut values = BTreeMap::new();
        event.record(&mut FieldValues {
            values: &mut values,
        });
        self.push(Captured {
            level: *event.metadata().level(),
            name: event.metadata().name().to_owned(),
            values,
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

/// The `scenario` span's fields, or a panic naming what was seen.
///
/// Looking the span up by name *and* level, rather than taking the first DEBUG
/// capture, is what keeps this honest: a span the runner forgot to open leaves
/// an empty capture list, not a wrong span, and the panic then says so.
pub(super) fn scenario_span_values(captured: &Captures) -> BTreeMap<String, String> {
    let all = seen(captured);
    let found = all
        .iter()
        .find(|item| item.level == Level::DEBUG && item.name == "scenario")
        .map(|item| item.values.clone());
    let Some(values) = found else {
        panic!("the run must open a `scenario` span at DEBUG; captured {all:?}");
    };
    values
}

/// The first capture carrying a field of this name, or a panic naming the rest.
pub(super) fn carrying<'a>(captured: &'a [Captured], field: &str) -> &'a Captured {
    let found = captured.iter().find(|item| item.carries(field));
    let Some(item) = found else {
        panic!(
            "no capture carried `{field}`; the captured fields were {:?}",
            captured_fields(captured),
        );
    };
    item
}

/// Every captured field, as name-to-value pairs, for a failure message.
fn captured_fields(captured: &[Captured]) -> BTreeMap<&str, &str> {
    captured
        .iter()
        .flat_map(|item| item.values.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .collect()
}

/// The trace-level per-step events, one per recorded invocation.
pub(super) fn per_step_events(captured: &[Captured]) -> Vec<&Captured> {
    captured
        .iter()
        .filter(|item| item.level == Level::TRACE && item.carries("status"))
        .collect()
}
