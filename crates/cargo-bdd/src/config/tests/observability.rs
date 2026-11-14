//! Span tests for the two configuration loaders.
//!
//! A capturing layer records each new span's name and field names. The
//! assertions pin both halves of the contract: every load opens one span, and
//! that span carries the command label only, so no configuration value can
//! reach a subscriber's logs.
//!
//! The capture installs a **global** default subscriber rather than a
//! thread-local one, because the callsites are process-wide state shared by
//! every test in this binary. A `DefaultCallsite` caches an `Interest` that
//! every thread reads, and `tracing_core` recomputes it from
//! `dispatcher::get_default`, which is the *calling thread's* default falling
//! back to the global one. `tracing::subscriber::with_default` installs only a
//! thread-local default and does not recompute anything, so a sibling test that
//! reaches one of these loaders concurrently caches its interest against the
//! process-wide `NoSubscriber` instead of this capture. Whichever loader loses
//! that race produces a span no layer sees. A global default makes every
//! thread's fallback the capture itself, so the cached interest cannot be
//! computed against a dispatcher that discards spans.

use std::{
    fmt,
    sync::{Arc, Mutex},
    thread::{self, ThreadId},
};

use clap::CommandFactory;
use ortho_config::MapEnv;
use tracing::{
    Subscriber,
    field::{Field, Visit},
    span::{Attributes, Id},
};
use tracing_subscriber::{
    layer::{Context, Layer, SubscriberExt},
    registry,
};

use super::{
    super::{SkippedConfig, StepsConfig, merge_steps, resolve_skipped},
    support::ConfigFixture,
};
use crate::cli::Cli;

/// One span as the capturing layer saw it.
#[derive(Clone, Debug, Eq, PartialEq)]
struct CapturedSpan {
    name: String,
    fields: Vec<String>,
    /// The thread that opened the span, so a case can ignore its siblings'.
    thread: ThreadId,
}

/// Shared sink for captured spans.
#[derive(Clone, Default)]
struct SpanCapture {
    spans: Arc<Mutex<Vec<CapturedSpan>>>,
}

impl SpanCapture {
    /// Take a copy of the spans captured so far on the calling thread.
    ///
    /// The capture sees every span the binary opens once it owns the global
    /// default, including those a concurrently running test opens. Reporting
    /// only the calling thread's spans keeps a case's verdict about its own
    /// loaders.
    fn recorded_by_this_thread(&self) -> Vec<CapturedSpan> {
        let Ok(spans) = self.spans.lock() else {
            panic!("the span capture mutex should not be poisoned");
        };
        let this_thread = thread::current().id();
        spans
            .iter()
            .filter(|span| span.thread == this_thread)
            .cloned()
            .collect()
    }
}

impl<S: Subscriber> Layer<S> for SpanCapture {
    /// Record a new span's name and field names.
    fn on_new_span(&self, attributes: &Attributes<'_>, _id: &Id, _context: Context<'_, S>) {
        let mut visitor = FieldNames::default();
        attributes.record(&mut visitor);
        let captured = CapturedSpan {
            name: attributes.metadata().name().to_owned(),
            fields: visitor.names,
            thread: thread::current().id(),
        };
        let Ok(mut spans) = self.spans.lock() else {
            panic!("the span capture mutex should not be poisoned");
        };
        spans.push(captured);
    }
}

/// Visitor that keeps field names and discards their values.
#[derive(Default)]
struct FieldNames {
    names: Vec<String>,
}

impl Visit for FieldNames {
    /// Keep the field's name and discard its value.
    fn record_debug(&mut self, field: &Field, _value: &dyn fmt::Debug) {
        self.names.push(field.name().to_owned());
    }
}

/// Parse `cargo-bdd <name>` and return that subcommand's matches.
fn subcommand_matches(name: &str) -> clap::ArgMatches {
    let Ok(matches) = Cli::command().try_get_matches_from(["cargo-bdd", name]) else {
        panic!("a supported subcommand should parse");
    };
    let Some(subcommand) = matches.subcommand_matches(name) else {
        panic!("the parsed command should carry its own matches");
    };
    subcommand.clone()
}

/// Each loader opens one span carrying only the command label.
#[test]
fn both_loaders_emit_one_bounded_configuration_span() {
    let capture = SpanCapture::default();
    let subscriber = registry().with(capture.clone());
    let steps_matches = subcommand_matches("steps");
    let skipped_matches = subcommand_matches("skipped");
    let fixture = ConfigFixture::new().expect("test setup should create the isolated fixture");
    let sources = fixture.sources(MapEnv::new());

    // A global default leaves no thread falling back to `NoSubscriber`, which
    // is what makes the capture deterministic; the module doc has the full
    // reason. Sibling tests keep running concurrently and may open spans of
    // their own, so the assertions below read back this thread's spans only.
    tracing::subscriber::set_global_default(subscriber)
        .expect("the unit-test binary should not install another global subscriber");
    // The merge result is irrelevant here: a failed load still opens and
    // closes its span, and the assertions below cover the span contract.
    let _ = merge_steps(&StepsConfig::default(), &steps_matches, &sources);
    let _ = resolve_skipped(&SkippedConfig::default(), &skipped_matches, &sources);

    let spans = capture.recorded_by_this_thread();
    assert_eq!(
        spans.len(),
        2,
        "each loader should open one span: {spans:?}"
    );
    for span in &spans {
        assert_eq!(span.name, "cargo_bdd.configuration");
        assert_eq!(
            span.fields,
            ["command"],
            "spans must not record configuration values: {span:?}"
        );
    }
}
