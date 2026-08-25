//! Logging predicates used by step-context diagnostics.

/// Report whether warnings for `target` are currently discarded.
///
/// The named predicate keeps the mirrored `eprintln!` fallback at its call
/// site a simple two-branch condition.
pub(super) fn warn_logging_is_disabled(target: &str) -> bool {
    !log::log_enabled!(target: target, log::Level::Warn)
}
