//! Logging predicates used by step-context diagnostics.

/// Report whether warning events are currently discarded.
///
/// The named predicate keeps the mirrored `eprintln!` fallback at its call
/// site a simple two-branch condition.
pub(super) fn warn_logging_is_disabled() -> bool { !log::log_enabled!(log::Level::Warn) }
