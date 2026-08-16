//! Wrapper-local argument binding helpers.
//!
//! Step wrappers need local bindings that never start with `_` so Clippy does
//! not flag `used_underscore_binding` when generated wrappers call user steps.
//! This module centralizes the `rstest_bdd_arg_{n}` naming scheme and keeps the
//! binding metadata next to each extracted argument.

use quote::format_ident;

use super::super::args::{Arg, DataTableArg, FixtureArg, StepArg, StepStructArg};

/// Wrapper-local argument bindings avoid leading underscores to keep Clippy happy.
#[derive(Copy, Clone)]
pub(in crate::codegen::wrapper) struct BoundFixtureArg<'a> {
    pub(super) arg: FixtureArg<'a>,
    pub(super) binding: &'a syn::Ident,
}

/// Wrapper-local binding for a typed step argument.
#[derive(Copy, Clone)]
pub(in crate::codegen::wrapper) struct BoundStepArg<'a> {
    pub(super) arg: StepArg<'a>,
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
