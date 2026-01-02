//! Shared helpers for rstest-bdd macro integration tests.

use super::args_impl::{Arg, ExtractedArgs};

pub(crate) fn fixture_count(args: &ExtractedArgs) -> usize {
    args.args
        .iter()
        .filter(|arg| matches!(arg, Arg::Fixture { .. }))
        .count()
}

pub(crate) fn step_arg_count(args: &ExtractedArgs) -> usize {
    args.args
        .iter()
        .filter(|arg| matches!(arg, Arg::Step { .. }))
        .count()
}

pub(crate) fn ordered_parameter_names(args: &ExtractedArgs) -> Vec<String> {
    args.args
        .iter()
        .map(|arg| match arg {
            Arg::Fixture { pat, .. }
            | Arg::Step { pat, .. }
            | Arg::StepStruct { pat, .. }
            | Arg::DataTable { pat, .. }
            | Arg::DocString { pat } => pat.to_string(),
        })
        .collect()
}

pub(crate) fn find_datatable(args: &ExtractedArgs) -> Option<&Arg> {
    args.args
        .iter()
        .find(|arg| matches!(arg, Arg::DataTable { .. }))
}

pub(crate) fn has_docstring(args: &ExtractedArgs) -> bool {
    args.args
        .iter()
        .any(|arg| matches!(arg, Arg::DocString { .. }))
}
