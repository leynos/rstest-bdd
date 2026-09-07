//! Warning-delivery route tests for the stderr fallback decision.

use std::sync::{
    Arc,
    Mutex,
    Once,
    atomic::{AtomicBool, Ordering},
};

use serial_test::serial;
use tracing::{
    Event,
    Level,
    Metadata,
    Subscriber,
    span::{Attributes, Id, Record},
};

use super::{super::warnings, scoped_subscriber};

/// Target the warning event and its fallback probe use; consumers that filter
/// or capture this exact target must keep seeing the warning.
const WARNING_TARGET: &str = "rstest_bdd::context";

/// Logger that answers `enabled` only for the established warning target.
///
/// A permanent process-global `log` logger models a consumer running
/// `env_logger` beside `tracing`, the coexistence the fallback contract must
/// respect. The `WARN`-level toggle restricts what it accepts so individual
/// tests can probe each delivery route.
struct ToggleableTargetLogger {
    accepts_warn: AtomicBool,
}

impl log::Log for ToggleableTargetLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Warn
            && metadata.target() == WARNING_TARGET
            && self.accepts_warn.load(Ordering::Relaxed)
    }

    fn log(&self, _: &log::Record<'_>) {}

    fn flush(&self) {}
}

static TOGGLEABLE_LOGGER: ToggleableTargetLogger = ToggleableTargetLogger {
    accepts_warn: AtomicBool::new(false),
};

static INIT_TOGGLEABLE_LOGGER: Once = Once::new();

/// Install the process-global test logger once.
///
/// It stays installed for the whole process because `log`'s global logger
/// cannot be replaced or removed; the toggle, not the installation, scopes
/// each test's behaviour. Installation also raises `log`'s global max level,
/// mirroring what `env_logger` does so `log_enabled!` reflects the logger's
/// own filter.
fn toggleable_logger() {
    INIT_TOGGLEABLE_LOGGER.call_once(|| {
        let _ = log::set_logger(&TOGGLEABLE_LOGGER);
        log::set_max_level(log::LevelFilter::Warn);
    });
}

/// RAII toggle accepting `WARN` records on [`WARNING_TARGET`] while held.
struct AcceptWarn;

impl AcceptWarn {
    fn enable() -> Self {
        TOGGLEABLE_LOGGER
            .accepts_warn
            .store(true, Ordering::Relaxed);
        log::set_max_level(log::LevelFilter::Warn);
        Self
    }
}

impl Drop for AcceptWarn {
    fn drop(&mut self) {
        TOGGLEABLE_LOGGER
            .accepts_warn
            .store(false, Ordering::Relaxed);
        log::set_max_level(log::LevelFilter::Error);
    }
}

/// Subscriber that records the target of the first delivered event.
struct TargetCapture {
    level: Level,
    captured: Arc<Mutex<Option<String>>>,
}

impl Subscriber for TargetCapture {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool { *metadata.level() <= self.level }

    fn new_span(&self, _: &Attributes<'_>) -> Id { Id::from_u64(1) }

    fn record(&self, _: &Id, _: &Record<'_>) {}

    fn record_follows_from(&self, _: &Id, _: &Id) {}

    fn event(&self, event: &Event<'_>) {
        let mut recorded = self
            .captured
            .lock()
            // Poison recovery keeps the capture working even if a panicking
            // assertion held the lock; the linter forbids `expect` here.
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if recorded.is_none() {
            *recorded = Some(event.metadata().target().to_owned());
        }
    }

    fn enter(&self, _: &Id) {}

    fn exit(&self, _: &Id) {}
}

/// Assert the emitted warning event carries the established context target.
///
/// Consumers capture or filter the exact `rstest_bdd::context` target, so the
/// emitting macro must keep using it instead of the `module_path!()` of the
/// private helper module.
#[test]
fn warning_event_carries_the_established_context_target() {
    let captured = Arc::new(Mutex::new(None::<String>));
    let _guard = tracing::subscriber::set_default(TargetCapture {
        level: Level::WARN,
        captured: Arc::clone(&captured),
    });

    warnings::emit_visible_warning("target probe");

    let recorded = captured
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    assert_eq!(
        recorded.as_deref(),
        Some(WARNING_TARGET),
        "the warning event must keep the established rstest_bdd::context target"
    );
}

/// Assert the warning reaches a `tracing` subscriber recording `WARN`.
///
/// The scoped subscriber models a listener that captures the event, so the
/// fallback probe must report a listener exists even though the process also
/// carries a `log` logger whose `WARN` toggle is on.
#[test]
#[serial]
fn warning_reaches_a_tracing_subscriber_despite_a_log_logger() {
    toggleable_logger();
    let _accept = AcceptWarn::enable();
    let (events, _guard) = scoped_subscriber(Level::WARN);

    assert!(
        !warnings::warn_reaches_no_listener(),
        "a recording tracing subscriber is a listener"
    );
    assert_eq!(
        events.load(Ordering::Relaxed),
        0,
        "the probe is a query; it must not record the event itself"
    );
}

/// Assert a `tracing` subscriber that filters `WARN` out is not a listener.
///
/// This is the false-negative the route gate exists to fix: with a dispatcher
/// present, tracing's `log` bridge never forwards, so an enabled `log` logger
/// must not mask the missing delivery. The mirror has to fire here.
#[test]
#[serial]
fn filtering_subscriber_with_log_logger_still_mirrors_to_stderr() {
    toggleable_logger();
    let _accept = AcceptWarn::enable();
    let (events, _guard) = scoped_subscriber(Level::ERROR);

    assert!(
        warnings::warn_reaches_no_listener(),
        "a WARN-filtering dispatcher leaves no delivery route; stderr must mirror"
    );
    assert_eq!(
        events.load(Ordering::Relaxed),
        0,
        "a filtering subscriber must not record the event"
    );
}

/// Assert the `log` bridge is the delivery route when no dispatcher was ever
/// set and the logger accepts `WARN` on the target.
#[test]
#[serial]
fn log_bridge_delivers_without_a_tracing_subscriber() {
    if tracing::dispatcher::has_been_set() {
        return;
    }
    toggleable_logger();
    let _accept = AcceptWarn::enable();

    assert!(
        !warnings::warn_reaches_no_listener(),
        "an enabled log logger is the delivery route before any dispatcher exists"
    );
}

/// Assert the fallback fires when no dispatcher was ever set and the logger
/// rejects `WARN` for the target.
#[test]
#[serial]
fn no_listener_without_dispatcher_or_log_listener() {
    if tracing::dispatcher::has_been_set() {
        return;
    }
    toggleable_logger();
    let _accept = AcceptWarn::enable();
    log::set_max_level(log::LevelFilter::Error);

    assert!(
        warnings::warn_reaches_no_listener(),
        "no tracing dispatcher and no WARN-level log listener means stderr mirrors"
    );
}
