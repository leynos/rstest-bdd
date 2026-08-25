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
    /// Stores the internal `arg` value.
    pub(super) arg: FixtureArg<'a>,
    /// Stores the internal `binding` value.
    pub(super) binding: &'a syn::Ident,
}

/// Wrapper-local binding for a typed step argument.
#[derive(Copy, Clone)]
pub(in crate::codegen::wrapper) struct BoundStepArg<'a> {
    /// Stores the internal `arg` value.
    pub(super) arg: StepArg<'a>,
    /// Stores the internal `binding` value.
    pub(super) binding: &'a syn::Ident,
}

#[derive(Copy, Clone)]
/// Internal data used by the macros implementation.
pub(in crate::codegen::wrapper) struct BoundStepStructArg<'a> {
    /// Stores the internal `arg` value.
    pub(super) arg: StepStructArg<'a>,
    /// Stores the internal `binding` value.
    pub(super) binding: &'a syn::Ident,
}

#[derive(Copy, Clone)]
/// Internal data used by the macros implementation.
pub(in crate::codegen::wrapper) struct BoundDataTableArg<'a> {
    /// Stores the internal `arg` value.
    pub(super) arg: DataTableArg<'a>,
    /// Stores the internal `binding` value.
    pub(super) binding: &'a syn::Ident,
}

#[derive(Copy, Clone)]
/// Internal data used by the macros implementation.
pub(in crate::codegen::wrapper) struct BoundDocStringArg<'a> {
    /// Stores the internal `binding` value.
    pub(super) binding: &'a syn::Ident,
}

/// Provides the internal `wrapper_binding_ident` operation.
pub(super) fn wrapper_binding_ident(index: usize) -> syn::Ident {
    format_ident!("rstest_bdd_arg_{index}")
}

/// Provides the internal `wrapper_binding_idents` operation.
pub(super) fn wrapper_binding_idents(args: &[Arg]) -> Vec<syn::Ident> {
    args.iter()
        .enumerate()
        .map(|(idx, _)| wrapper_binding_ident(idx))
        .collect()
}
