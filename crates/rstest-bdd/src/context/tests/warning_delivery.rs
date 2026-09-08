//! Warning-delivery route tests for the stderr fallback decision.

use std::{
    process::{Command, Stdio},
    sync::{
        Arc,
        Mutex,
        Once,
        atomic::{AtomicBool, Ordering},
    },
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

/// Environment variable marking a re-executed probe child process.
///
/// The body of the targeted probe emits the mirror only when this variable is
/// set, so a normal run of the registered test stays a quiet no-op.
const CHILD_ENV: &str = "RSTEST_BDD_WARNING_PROBE_CHILD";

/// Full test path of the probe body, passed to `--exact` on re-execution.
///
/// The `--exact` form avoids libtest's substring filter matching helper
/// functions that share the probe body's name as a prefix.
const TARGETED_TEST_ARG: &str = concat!(
    "context::tests::warning_delivery::",
    "run_filtering_subscriber_probe"
);

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

/// Message emitted and matched by the subprocess fallback test; a unique
/// constant keeps a concurrent unrelated run's stderr from matching.
const FALLBACK_PROBE_MESSAGE: &str =
    "rstest-bdd fallback probe: filtering subscriber must mirror to stderr";

/// Child-mode body of the subprocess fallback test, registered as a test.
///
/// The parent test re-executes this binary with `--exact` and this test's
/// path, so the function must stay `#[test]`-registered or the child would
/// run zero tests. In a normal run the child marker is absent and the body
/// returns immediately; in the child it installs the probe setup — the
/// toggleable log logger, `AcceptWarn`, and a scoped `ERROR`-only subscriber
/// — emits the probe message, and asserts the subscriber stayed out of the
/// delivery route. The mirror then reaches this process's stderr, which the
/// parent captures.
#[test]
#[serial]
fn run_filtering_subscriber_probe() {
    if std::env::var_os(CHILD_ENV).is_none() {
        return;
    }
    toggleable_logger();
    let _accept = AcceptWarn::enable();
    let (events, _guard) = scoped_subscriber(Level::ERROR);

    warnings::emit_visible_warning(FALLBACK_PROBE_MESSAGE);

    // The subscriber must stay out of the delivery route; assert it directly
    // rather than leaving the probe's behaviour unobserved.
    assert_eq!(
        events.load(Ordering::Relaxed),
        0,
        "a filtering subscriber must not record the event"
    );
}

/// Re-execute the running test binary for exactly one test, returning its
/// captured stderr and success flag.
///
/// The env marker plus `--exact` keeps the child from running any other test,
/// so only the child's own stderr (including the mirrored probe message)
/// reaches the parent's capture.
fn run_child_probe() -> (String, bool) {
    let Ok(executable) = std::env::current_exe() else {
        return (
            String::from("could not resolve the current test binary path"),
            false,
        );
    };
    let Ok(output) = Command::new(executable)
        .env(CHILD_ENV, "1")
        .args([
            "--exact",
            TARGETED_TEST_ARG,
            "--nocapture",
            "--test-threads=1",
        ])
        .stderr(Stdio::piped())
        .stdout(Stdio::null())
        .stdin(Stdio::null())
        .output()
    else {
        return (
            String::from("could not spawn the re-executed test binary"),
            false,
        );
    };
    (
        String::from_utf8_lossy(&output.stderr).into_owned(),
        output.status.success(),
    )
}

/// Assert a `tracing` subscriber that filters `WARN` out still mirrors the
/// warning to stderr.
///
/// This is the false-negative the route gate exists to fix: with a dispatcher
/// present, tracing's `log` bridge never forwards, so an enabled `log` logger
/// must not mask the missing delivery. The mirror is verified directly in a
/// subprocess so the process-global logger stays isolated from parallel
/// unit tests; the sibling in-process test keeps the predicate assertion.
#[test]
fn filtering_subscriber_with_log_logger_still_mirrors_to_stderr() {
    let (stderr, child_success) = run_child_probe();
    assert!(child_success, "child probe must pass; stderr: {stderr}");
    assert!(
        stderr.contains(FALLBACK_PROBE_MESSAGE),
        "the stderr mirror must carry the probe message"
    );
}

/// Assert the in-process predicate behind the stderr mirror still reports no
/// listener under a WARN-filtering dispatcher.
///
/// Direct stderr capture requires a subprocess because the global `log`
/// logger cannot be replaced, so this process-local companion test retains
/// the predicate assertion the child body depends on.
#[test]
#[serial]
fn filtering_subscriber_leaves_no_delivery_route() {
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
