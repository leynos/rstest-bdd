//! Tests for step-context warning delivery.
//!
//! These cover the routes described in [`crate::context::warnings`]: a warning
//! must reach an active `tracing` subscriber, must be withheld from one that
//! filters `WARN` out, and must fall back to stderr only when no listener
//! exists on either route.

use super::scoped_subscriber;
use crate::context::warnings::{emit_visible_warning, warn_reaches_no_listener};
use std::sync::atomic::Ordering;
use tracing::Level;

const MESSAGE: &str = "ambiguous step-return override was ignored";

#[test]
fn warning_reaches_an_active_subscriber() {
    let (events, _guard) = scoped_subscriber(Level::WARN);

    emit_visible_warning(MESSAGE);

    assert_eq!(
        events.load(Ordering::Relaxed),
        1,
        "a subscriber recording WARN should receive the warning"
    );
}

#[test]
fn warning_is_withheld_from_a_filtering_subscriber() {
    let (events, _guard) = scoped_subscriber(Level::ERROR);

    emit_visible_warning(MESSAGE);

    assert_eq!(
        events.load(Ordering::Relaxed),
        0,
        "a subscriber filtering WARN out should receive nothing"
    );
}

#[test]
fn an_active_subscriber_suppresses_the_stderr_fallback() {
    let (_events, _guard) = scoped_subscriber(Level::WARN);

    assert!(
        !warn_reaches_no_listener(),
        "the stderr fallback must stay quiet while a subscriber records WARN"
    );
}

/// No `log` logger is installed anywhere in this crate, so a subscriber that
/// filters `WARN` out leaves both delivery routes without a listener and the
/// stderr fallback becomes the only way the warning is surfaced.
#[test]
fn a_filtering_subscriber_leaves_the_stderr_fallback_as_the_only_route() {
    let (_events, _guard) = scoped_subscriber(Level::ERROR);

    assert!(
        warn_reaches_no_listener(),
        "with WARN filtered out and no log logger, stderr must carry the warning"
    );
}
