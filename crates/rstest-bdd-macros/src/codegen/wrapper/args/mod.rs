//! Argument extraction primitives shared by the wrapper generator.
//!
//! Arguments are stored as a single [`Arg`] enum so later stages can iterate
//! over the original function signature without juggling parallel vectors for
//! fixtures, captures, and Gherkin-specific parameters.

use std::{collections::HashSet, fmt};

mod classify;
mod extract;

pub use extract::extract_args;

/// Everything required to describe a single step-function argument.
#[derive(Clone)]
pub enum Arg {
    Fixture {
        pat: syn::Ident,
        name: syn::Ident,
        ty: syn::Type,
    },
    Step {
        pat: syn::Ident,
        ty: syn::Type,
    },
    StepStruct {
        pat: syn::Ident,
        ty: syn::Type,
    },
    DataTable {
        pat: syn::Ident,
        ty: syn::Type,
    },
    DocString {
        pat: syn::Ident,
    },
}

#[derive(Clone, Copy)]
pub struct StepStructArg<'a> {
    pub pat: &'a syn::Ident,
    pub ty: &'a syn::Type,
}

#[derive(Clone, Copy)]
pub struct DataTableArg<'a> {
    pub pat: &'a syn::Ident,
    pub ty: &'a syn::Type,
}

#[derive(Clone, Copy)]
pub struct DocStringArg<'a> {
    pub pat: &'a syn::Ident,
}

#[expect(
    clippy::use_self,
    reason = "enum variant paths remain explicit in match arms"
)]
impl Arg {
    /// Identifier bound in the user function signature.
    pub fn pat(&self) -> &syn::Ident {
        match self {
            Arg::Fixture { pat, .. }
            | Arg::Step { pat, .. }
            | Arg::StepStruct { pat, .. }
            | Arg::DataTable { pat, .. }
            | Arg::DocString { pat } => pat,
        }
    }

    pub fn as_step_struct(&self) -> Option<StepStructArg<'_>> {
        match self {
            Arg::StepStruct { pat, ty } => Some(StepStructArg { pat, ty }),
            _ => None,
        }
    }

    pub fn as_datatable(&self) -> Option<DataTableArg<'_>> {
        match self {
            Arg::DataTable { pat, ty } => Some(DataTableArg { pat, ty }),
            _ => None,
        }
    }

    pub fn as_docstring(&self) -> Option<DocStringArg<'_>> {
        match self {
            Arg::DocString { pat } => Some(DocStringArg { pat }),
            _ => None,
        }
    }
}

#[expect(
    clippy::use_self,
    reason = "enum variant paths remain explicit in formatter"
)]
impl fmt::Debug for Arg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Arg::Fixture { pat, name, ty } => f
                .debug_struct("Fixture")
                .field("pat", pat)
                .field("name", name)
                .field("ty", ty)
                .finish(),
            Arg::Step { pat, ty } => f
                .debug_struct("Step")
                .field("pat", pat)
                .field("ty", ty)
                .finish(),
            Arg::StepStruct { pat, ty } => f
                .debug_struct("StepStruct")
                .field("pat", pat)
                .field("ty", ty)
                .finish(),
            Arg::DataTable { pat, ty } => f
                .debug_struct("DataTable")
                .field("pat", pat)
                .field("ty", ty)
                .finish(),
            Arg::DocString { pat } => f.debug_struct("DocString").field("pat", pat).finish(),
        }
    }
}

/// Ordered arguments plus quick-look indexes for unique variants.
#[derive(Clone, Default)]
pub struct ExtractedArgs {
    pub args: Vec<Arg>,
    pub(super) step_struct_idx: Option<usize>,
    pub(super) datatable_idx: Option<usize>,
    pub(super) docstring_idx: Option<usize>,
    pub(super) blocked_placeholders: HashSet<String>,
}

impl ExtractedArgs {
    pub fn push(&mut self, arg: Arg) -> usize {
        let idx = self.args.len();
        self.args.push(arg);
        idx
    }

    pub fn fixtures(&self) -> impl Iterator<Item = &Arg> {
        self.args
            .iter()
            .filter(|arg| matches!(arg, Arg::Fixture { .. }))
    }

    pub fn step_args(&self) -> impl Iterator<Item = &Arg> {
        self.args
            .iter()
            .filter(|arg| matches!(arg, Arg::Step { .. }))
    }

    pub fn step_struct(&self) -> Option<StepStructArg<'_>> {
        self.step_struct_idx
            .and_then(|idx| self.args.get(idx))
            .and_then(Arg::as_step_struct)
    }
}

impl fmt::Debug for ExtractedArgs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut dbg = f.debug_struct("ExtractedArgs");
        dbg.field("count", &self.args.len());
        if !self.args.is_empty() {
            let labels: Vec<_> = self
                .args
                .iter()
                .map(|arg| match arg {
                    Arg::Fixture { pat, .. } => format!("fixture {pat}"),
                    Arg::Step { pat, .. } => format!("step {pat}"),
                    Arg::StepStruct { pat, .. } => format!("step_struct {pat}"),
                    Arg::DataTable { pat, .. } => format!("datatable {pat}"),
                    Arg::DocString { pat } => format!("docstring {pat}"),
                })
                .collect();
            dbg.field("args", &labels);
        }
        dbg.field("step_struct_idx", &self.step_struct_idx)
            .field("datatable_idx", &self.datatable_idx)
            .field("docstring_idx", &self.docstring_idx)
            .field("blocked_placeholders", &self.blocked_placeholders)
            .finish()
    }
}
