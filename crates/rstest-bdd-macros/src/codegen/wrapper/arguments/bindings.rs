//! Wrapper-local argument binding helpers.

use super::super::args::{Arg, DataTableArg, StepStructArg};
use quote::format_ident;

/// Wrapper-local argument bindings avoid leading underscores to keep Clippy happy.
#[derive(Copy, Clone)]
pub(in crate::codegen::wrapper) struct BoundArg<'a> {
    pub(super) arg: &'a Arg,
    pub(super) binding: &'a syn::Ident,
}

#[derive(Copy, Clone)]
pub(in crate::codegen::wrapper) struct BoundStepStructArg<'a> {
    pub(super) arg: StepStructArg<'a>,
    pub(super) binding: &'a syn::Ident,
}

#[derive(Copy, Clone)]
pub(in crate::codegen::wrapper) struct BoundDataTableArg<'a> {
    pub(super) arg: DataTableArg<'a>,
    pub(super) binding: &'a syn::Ident,
}

#[derive(Copy, Clone)]
pub(in crate::codegen::wrapper) struct BoundDocStringArg<'a> {
    pub(super) binding: &'a syn::Ident,
}

pub(super) fn wrapper_binding_ident(index: usize) -> syn::Ident {
    format_ident!("rstest_bdd_arg_{index}")
}

pub(super) fn wrapper_binding_idents(args: &[Arg]) -> Vec<syn::Ident> {
    args.iter()
        .enumerate()
        .map(|(idx, _)| wrapper_binding_ident(idx))
        .collect()
}
