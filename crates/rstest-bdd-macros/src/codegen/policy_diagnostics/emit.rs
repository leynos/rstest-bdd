//! Toolchain-aware emission for policy diagnostics.
//!
//! The two channels are mutually exclusive by construction: on nightly the
//! stable token builder returns nothing and the native emitter runs, and on
//! stable the native emitter is a no-op and the token builder produces the
//! deprecated-item markers. Unit tests take the stable token path and never
//! call `proc_macro::Diagnostic`.
//!
//! This module is not `codegen::scenario::emit`, which assembles the generated
//! test item; this one owns only diagnostic rendering.

#[cfg(not(rstest_bdd_nightly))]
use proc_macro2::Ident;
use proc_macro2::{Span, TokenStream as TokenStream2};

/// The stable marker a rendered warning uses.
///
/// The marker name is the first thing a user reads in the compiler output, so
/// each code gets a headline of its own rather than a shared name.
#[cfg(not(rstest_bdd_nightly))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Marker {
    /// W001, the non-canonical first-party path.
    UnrecognizedAdapterPath,
    /// W002, the evidenced harness path with no attribute policy.
    MissingAttributePolicy,
}

#[cfg(not(rstest_bdd_nightly))]
impl Marker {
    /// The private generated struct that carries the deprecated note.
    ///
    /// Each code gets its own headline because the name is the first thing a
    /// user reads after the words "use of deprecated unit struct".
    fn struct_ident(self, span: Span) -> Ident {
        let name = match self {
            Self::UnrecognizedAdapterPath => "RstestBddUnrecognizedAdapterPath",
            Self::MissingAttributePolicy => "RstestBddMissingAttributePolicy",
        };
        Ident::new(name, span)
    }
}

/// One warning, ready for either channel.
#[derive(Clone, Debug)]
pub(crate) struct RenderedWarning {
    /// The complete message the user reads.
    pub(crate) message: String,
    /// The span the warning points at.
    pub(crate) span: Span,
    /// The stable marker this warning uses.
    #[cfg(not(rstest_bdd_nightly))]
    pub(crate) marker: Marker,
}

/// Emit each warning through the native diagnostic API when it exists.
#[cfg(all(rstest_bdd_nightly, not(test)))]
pub(crate) fn emit_native(warnings: &[RenderedWarning]) {
    for warning in warnings {
        proc_macro::Diagnostic::spanned(
            warning.span.unwrap(),
            proc_macro::Level::Warning,
            warning.message.clone(),
        )
        .emit();
    }
}

/// Do nothing where the native diagnostic API is unavailable.
#[cfg(any(not(rstest_bdd_nightly), test))]
pub(crate) fn emit_native(_: &[RenderedWarning]) {}

/// Build the stable deprecated-item tokens for each warning.
///
/// Stable procedural macros cannot emit warnings, so each warning becomes a
/// sibling `const _` block that names a `#[deprecated]` unit struct whose note
/// carries the message. The user sees a `deprecated` warning at the supplied
/// path, and `deny(deprecated)` turns it into a pinned error.
#[cfg(not(rstest_bdd_nightly))]
pub(crate) fn warning_tokens(warnings: &[RenderedWarning]) -> TokenStream2 {
    let blocks = warnings.iter().map(|warning| {
        let message = &warning.message;
        let span = warning.span;
        let marker = warning.marker.struct_ident(span);
        quote::quote_spanned! {span=>
            const _: () = {
                #[deprecated(note = #message)]
                struct #marker;
                let _ = #marker;
            };
        }
    });
    quote::quote! { #(#blocks)* }
}

/// Nightly routes the same guidance through the native channel instead.
#[cfg(rstest_bdd_nightly)]
pub(crate) fn warning_tokens(_: &[RenderedWarning]) -> TokenStream2 { TokenStream2::new() }
