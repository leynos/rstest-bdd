//! Cargo rebuild-dependency tracking for bound `.feature` files (ADR-010,
//! roadmap item 10.3.3).
//!
//! Cargo decides whether to recompile a crate from its dep-info file, and
//! `rustc` only records a file there when it was pulled in through
//! `include_str!`, `include_bytes!`, `include!`, or a build script's
//! `rerun-if-changed` directive. A procedural macro that opens a `.feature`
//! file with ordinary filesystem calls is invisible to that machinery, so a
//! `.feature`-only edit silently skips recompilation — in a testing framework
//! the worst possible failure mode.
//!
//! The mechanism this module implements is the *tracking binding* (Decision
//! D0 in the 10.3.3 ExecPlan): each macro emits, once per bound feature file,
//! an anonymous `const` whose value is `include_bytes!(concat!(
//! env!("CARGO_MANIFEST_DIR"), "/", <relative literal>))`. The deferred path
//! construction keeps the emitted token stream free of absolute paths, and
//! rustc registers the file in dep-info without the file contents or the
//! path ever landing in a binary (the anonymous `const` is unused and is
//! elided; measured in `ExecPlan` transcripts A and B).
//!
//! The regression tests that guard this behaviour are the two scenario-bound
//! tests in `crates/rstest-bdd/tests/feature_rebuild_invalidation.rs` (the
//! dep-info contract and the edit-and-rebuild experiment) and the token-shape
//! tests in the sibling `tests` module.
//!
//! Milestone 1 note: this module is scaffolded with
//! [`feature_tracking_item`] returning an empty token stream — the
//! pre-fix "no tracking item is emitted" state that makes the Milestone 1
//! red-stage tests fail for their stated reason while keeping the crate
//! lint-clean. Milestone 2 replaces the body with the real binding.

use proc_macro2::TokenStream;

/// Resolves `path` and emits the tracking item or a `compile_error!`.
///
/// This is the only place that decides what happens on failure, so the
/// decision cannot be forgotten at a call site. Milestone 1 scaffold: emits
/// nothing, so callers observe the pre-fix behaviour.
pub(crate) fn feature_tracking_item(
    _path: &std::path::Path,
    _span: proc_macro2::Span,
) -> TokenStream {
    TokenStream::new()
}

#[cfg(test)]
mod tests;
