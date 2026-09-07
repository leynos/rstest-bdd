//! Warning delivery for step-context diagnostics.
//!
//! A warning raised while resolving step-return overrides must reach the
//! developer even when the consuming test binary configures no logging at all,
//! which is the common case for a test framework. Emission therefore goes
//! through `tracing` (ADR-020), and a mirrored `eprintln!` covers the case
//! where no listener exists, so the warning is surfaced exactly once.

/// Target every step-context warning is emitted and probed under.
///
/// The macros here pass this explicitly rather than relying on
/// `module_path!()`, which would resolve to `rstest_bdd::context::warnings`
/// inside this module. Consumers that capture or filter the established
/// `rstest_bdd::context` target would otherwise stop matching the structured
/// event, so emission and the fallback probe must share this constant.
const WARNING_TARGET: &str = "rstest_bdd::context";

/// Emit `message` as a warning, mirroring it to stderr when no logging
/// listener would otherwise receive it.
pub(super) fn emit_visible_warning(message: &str) {
    tracing::warn!(target: WARNING_TARGET, "{message}");
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
/// `tracing` delivers such an event by one of two routes, selected by whether
/// a dispatcher has ever been installed:
///
/// - when no dispatcher has ever been set, as a `log` record through the compatibility bridge
///   enabled by tracing's `log` feature; or
/// - otherwise, to the current `tracing` dispatcher, which may also be the built-in no-op when only
///   a scoped subscriber has been removed.
///
/// The route is chosen with [`tracing::dispatcher::has_been_set`], mirroring
/// the gate in tracing's own `log` bridge: once a dispatcher exists, the
/// bridge never forwards again, so an enabled `log` logger must not be counted
/// as a listener on that route. Doing so would report delivery that never
/// happens and suppress the mirrored `eprintln!` for a warning that reaches
/// no one.
///
/// The probe is colocated with [`emit_visible_warning`] so both resolve the
/// same explicit [`WARNING_TARGET`] and are therefore subject to identical
/// filtering.
pub(super) fn warn_reaches_no_listener() -> bool {
    if tracing::dispatcher::has_been_set() {
        !tracing::event_enabled!(target: WARNING_TARGET, tracing::Level::WARN)
    } else {
        !log::log_enabled!(target: WARNING_TARGET, log::Level::Warn)
    }
}
