//! Configuration shared across wrapper generation helpers.

use super::args::{CallArg, DataTableArg, DocStringArg, FixtureArg, StepArg};

pub(crate) struct WrapperConfig<'a> {
    pub(crate) ident: &'a syn::Ident,
    pub(crate) fixtures: &'a [FixtureArg],
    pub(crate) step_args: &'a [StepArg],
    pub(crate) datatable: Option<&'a DataTableArg>,
    pub(crate) docstring: Option<&'a DocStringArg>,
    pub(crate) pattern: &'a syn::LitStr,
    pub(crate) keyword: crate::StepKeyword,
    pub(crate) call_order: &'a [CallArg],
}
