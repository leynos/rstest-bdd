//! Argument extraction and classification helpers for wrapper generation.

mod classify;
mod extract;

pub use extract::extract_args;

/// Fixture argument extracted from a step function.
#[derive(Debug, Clone)]
pub struct FixtureArg {
    pub pat: syn::Ident,
    pub name: syn::Ident,
    pub ty: syn::Type,
}

/// Non-fixture argument extracted from a step function.
#[derive(Debug, Clone)]
pub struct StepArg {
    pub pat: syn::Ident,
    pub ty: syn::Type,
}

/// Struct-based step argument populated by parsing all placeholders.
#[derive(Debug, Clone)]
pub struct StepArgStruct {
    pub pat: syn::Ident,
    pub ty: syn::Type,
}

/// Represents an argument for a Gherkin data table step function.
///
/// The [`ty`] field stores the Rust type of the argument. This enables
/// type-specific logic such as code generation, validation, or transformation
/// based on the argument's type. Documenting the type here clarifies its role in
/// macro expansion and helps future maintainers understand how type information
/// is propagated.
///
/// # Fields
/// - `pat`: The identifier pattern for the argument.
/// - `ty`: The Rust type of the argument, used for type-specific logic and code generation.
#[derive(Debug, Clone)]
pub struct DataTableArg {
    pub pat: syn::Ident,
    pub ty: syn::Type,
}

/// Gherkin doc string argument extracted from a step function.
#[derive(Debug, Clone)]
pub struct DocStringArg {
    pub pat: syn::Ident,
}

/// Argument ordering as declared in the step function signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallArg {
    Fixture(usize),
    StepArg(usize),
    StepStruct,
    DataTable,
    DocString,
}

/// Collections of arguments extracted from a step function signature.
#[derive(Clone)]
pub struct ExtractedArgs {
    pub fixtures: Vec<FixtureArg>,
    pub step_args: Vec<StepArg>,
    pub step_struct: Option<StepArgStruct>,
    pub datatable: Option<DataTableArg>,
    pub docstring: Option<DocStringArg>,
    pub call_order: Vec<CallArg>,
}

/// References to extracted arguments for ordered processing.
#[derive(Clone, Copy)]
pub(crate) struct ArgumentCollections<'a> {
    pub(crate) fixtures: &'a [FixtureArg],
    pub(crate) step_args: &'a [StepArg],
    pub(crate) step_struct: Option<&'a StepArgStruct>,
    pub(crate) datatable: Option<&'a DataTableArg>,
    pub(crate) docstring: Option<&'a DocStringArg>,
}

impl std::fmt::Debug for ExtractedArgs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExtractedArgs")
            .field("fixtures", &self.fixtures.len())
            .field("step_args", &self.step_args.len())
            .field("step_struct", &self.step_struct.is_some())
            .field("datatable", &self.datatable.is_some())
            .field("docstring", &self.docstring.is_some())
            .field("call_order", &self.call_order)
            .finish()
    }
}
