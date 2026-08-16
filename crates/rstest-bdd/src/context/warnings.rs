//! Warning delivery for step-context diagnostics.
//!
//! A warning raised while resolving step-return overrides must reach the
//! developer even when the consuming test binary configures no logging at all,
//! which is the common case for a test framework. Emission therefore goes
//! through `tracing` (ADR-020), and a mirrored `eprintln!` covers the case
//! where no listener exists, so the warning is surfaced exactly once.

/// Emit `message` as a warning, mirroring it to stderr when no logging
/// listener would otherwise receive it.
pub(super) fn emit_visible_warning(message: &str) {
    tracing::warn!("{message}");
    #[expect(
        clippy::print_stderr,
        reason = "surface step-context warnings when no logging listener exists"
    )]
    if warn_reaches_no_listener() {
        eprintln!("{message}");
    }
}

/// Report whether a `WARN` event raised here would reach no listener at all.
///
/// `tracing` delivers such an event by one of two routes:
///
/// - to the active `tracing` subscriber, when one is installed; or
/// - as a `log` record through the compatibility bridge enabled by tracing's
///   `log` feature, which fires only while no subscriber has ever been set.
///
/// Both routes are probed, so the mirrored `eprintln!` fires only when neither
/// has a listener. A consumer whose subscriber filters `WARN` out has asked for
/// silence on that route, and the probe honours that by reporting a listener
/// only where the event would actually be recorded.
///
/// The probe is colocated with [`emit_visible_warning`] so both resolve the
/// same `module_path!()` target and are therefore subject to identical
/// filtering.
pub(super) fn warn_reaches_no_listener() -> bool {
    !tracing::event_enabled!(tracing::Level::WARN) && !log::log_enabled!(log::Level::Warn)
}
