//! Toolchain-aware warning emission for procedural macro diagnostics.
//!
//! Callers supply a source span, message, and optional note. Non-test builds
//! selected by the `rstest_bdd_nightly` build-script configuration emit a
//! native compiler warning. Stable and test builds deliberately do nothing.

use proc_macro2::Span;

/// Emit a procedural-macro warning when native diagnostics are available.
#[cfg(all(rstest_bdd_nightly, not(test)))]
pub(crate) fn emit_warning(span: Span, message: String, note: Option<&str>) {
    let diagnostic =
        proc_macro::Diagnostic::spanned(span.unwrap(), proc_macro::Level::Warning, message);
    if let Some(note) = note {
        diagnostic.note(note).emit();
    } else {
        diagnostic.emit();
    }
}

/// Keep warning call sites portable on stable toolchains and in unit tests.
#[cfg(any(not(rstest_bdd_nightly), test))]
pub(crate) fn emit_warning(_: Span, _: String, _: Option<&str>) {}
