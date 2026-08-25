//! Shared metadata and prepared token collections for wrapper arguments.

use proc_macro2::TokenStream as TokenStream2;

/// Metadata identifying the step for which argument code is generated.
#[derive(Copy, Clone)]
pub(in super::super) struct StepMeta<'a> {
    /// The declared step pattern.
    pub(in super::super) pattern: &'a syn::LitStr,
    /// The identifier of the step function.
    pub(in super::super) ident: &'a syn::Ident,
}

/// Generated argument declarations and parsing tokens for a wrapper function.
pub(in super::super) struct PreparedArgs {
    /// Fixture declarations evaluated before invoking the step.
    pub(in super::super) declares: Vec<TokenStream2>,
    /// Capture parsing expressions for ordinary step arguments.
    pub(in super::super) step_arg_parses: Vec<TokenStream2>,
    /// The optional generated step-struct declaration.
    pub(in super::super) step_struct_decl: Option<TokenStream2>,
    /// The optional generated data-table declaration.
    pub(in super::super) datatable_decl: Option<TokenStream2>,
    /// The optional generated doc-string declaration.
    pub(in super::super) docstring_decl: Option<TokenStream2>,
    /// Lints expected in the generated wrapper implementation.
    pub(in super::super) expect_lints: Vec<syn::Path>,
    /// Whether step argument parsing strips string-placeholder quotes.
    pub(in super::super) has_step_arg_quote_strip: bool,
}
