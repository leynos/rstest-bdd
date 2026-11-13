//! Span tests for the two configuration loaders.
//!
//! A capturing layer records each new span's name and field names. The
//! assertions pin both halves of the contract: every load opens one span, and
//! that span carries the command label only, so no configuration value can
//! reach a subscriber's logs.

use std::{
    fmt,
    sync::{Arc, Mutex},
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
}

/// Shared sink for captured spans.
#[derive(Clone, Default)]
struct SpanCapture {
    spans: Arc<Mutex<Vec<CapturedSpan>>>,
}

impl SpanCapture {
    /// Take a copy of the spans captured so far.
    fn recorded(&self) -> Vec<CapturedSpan> {
        let Ok(spans) = self.spans.lock() else {
            panic!("the span capture mutex should not be poisoned");
        };
        spans.clone()
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

    tracing::subscriber::with_default(subscriber, || {
        // The merge result is irrelevant here: a failed load still opens and
        // closes its span, and the assertions below cover the span contract.
        let _ = merge_steps(&StepsConfig::default(), &steps_matches, &sources);
        let _ = resolve_skipped(&SkippedConfig::default(), &skipped_matches, &sources);
    });

    let spans = capture.recorded();
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
