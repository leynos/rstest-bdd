//! Property tests for typed wrapper-argument views and bindings.
//!
//! The example tests cover representative signatures. These properties vary
//! the full argument sequence to ensure filtering and binding preserve the
//! associations established by the wrapper generator for every arrangement.

use proptest::prelude::*;
use syn::{Ident, parse_quote};

use super::helpers::{bind_args, bind_fixture_args};
use crate::codegen::wrapper::{
    args::{Arg, ExtractedArgs},
    arguments::{BoundFixtureArg, BoundStepArg},
};

/// Kinds represented in the generated argument sequence.
#[derive(Clone, Copy, Debug)]
enum ArgKind {
    /// A fixture parameter.
    Fixture,
    /// A captured step parameter.
    Step,
    /// A `#[step_args]` parameter.
    StepStruct,
    /// A data-table parameter.
    DataTable,
    /// A docstring parameter.
    DocString,
}

/// Produce the argument kind represented by a bounded test value.
fn arg_kind(value: u8) -> ArgKind {
    match value {
        0 => ArgKind::Fixture,
        1 => ArgKind::Step,
        2 => ArgKind::StepStruct,
        3 => ArgKind::DataTable,
        _ => ArgKind::DocString,
    }
}

/// Build an argument whose names are unique and encode its source position.
fn argument_at(kind: ArgKind, index: usize) -> Arg {
    let ident = |prefix| Ident::new(&format!("{prefix}_{index}"), proc_macro2::Span::call_site());
    match kind {
        ArgKind::Fixture => Arg::Fixture {
            pat: ident("fixture_pat"),
            name: ident("fixture"),
            ty: parse_quote!(String),
        },
        ArgKind::Step => Arg::Step {
            pat: ident("step"),
            ty: parse_quote!(usize),
        },
        ArgKind::StepStruct => Arg::StepStruct {
            pat: ident("step_struct"),
            ty: parse_quote!(Args),
        },
        ArgKind::DataTable => Arg::DataTable {
            pat: ident("datatable"),
            ty: parse_quote!(Vec<Vec<String>>),
        },
        ArgKind::DocString => Arg::DocString {
            pat: ident("docstring"),
        },
    }
}

/// Build the argument sequence used by one generated test case.
fn arguments(kinds: &[u8]) -> Vec<Arg> {
    kinds
        .iter()
        .enumerate()
        .map(|(index, kind)| argument_at(arg_kind(*kind), index))
        .collect()
}

/// Return fixture names in their source order.
fn fixture_names(args: &[Arg]) -> Vec<String> {
    args.iter()
        .filter_map(Arg::as_fixture)
        .map(|fixture| fixture.name.to_string())
        .collect()
}

/// Return step names in their source order.
fn step_names(args: &[Arg]) -> Vec<String> {
    args.iter()
        .filter_map(Arg::as_step)
        .map(|step| step.pat.to_string())
        .collect()
}

/// Build unique bindings whose values encode their matching filtered position.
fn bindings(prefix: &str, count: usize) -> Vec<Ident> {
    (0..count)
        .map(|index| Ident::new(&format!("{prefix}_{index}"), proc_macro2::Span::call_site()))
        .collect()
}

/// Assert that fixture bindings remain paired with their originating fixtures.
fn assert_fixture_bindings(bound: &[BoundFixtureArg<'_>], expected_names: &[String]) {
    let actual = bound
        .iter()
        .enumerate()
        .map(|(index, binding)| {
            (
                binding.arg.name.to_string(),
                binding.binding.to_string(),
                index,
            )
        })
        .collect::<Vec<_>>();
    let expected = expected_names
        .iter()
        .enumerate()
        .map(|(index, name)| (name.clone(), format!("fixture_binding_{index}"), index))
        .collect::<Vec<_>>();
    assert_eq!(
        actual, expected,
        "fixture bindings must retain source associations"
    );
}

/// Assert that step bindings remain paired with their originating step arguments.
fn assert_step_bindings(bound: &[BoundStepArg<'_>], expected_names: &[String]) {
    let actual = bound
        .iter()
        .enumerate()
        .map(|(index, binding)| {
            (
                binding.arg.pat.to_string(),
                binding.binding.to_string(),
                index,
            )
        })
        .collect::<Vec<_>>();
    let expected = expected_names
        .iter()
        .enumerate()
        .map(|(index, name)| (name.clone(), format!("step_binding_{index}"), index))
        .collect::<Vec<_>>();
    assert_eq!(
        actual, expected,
        "step bindings must retain source associations"
    );
}

proptest! {
    /// Typed views filter only their matching variant and preserve source order.
    #[test]
    fn typed_views_preserve_filtered_order_and_variant_boundaries(kinds in prop::collection::vec(0u8..5, 0..32)) {
        let args = arguments(&kinds);
        let extracted = ExtractedArgs {
            args: args.clone(),
            ..ExtractedArgs::default()
        };
        let expected_fixtures = fixture_names(&args);
        let expected_steps = step_names(&args);
        let actual_fixtures = extracted
            .fixtures()
            .map(|fixture| fixture.name.to_string())
            .collect::<Vec<_>>();
        let actual_steps = extracted
            .step_args()
            .map(|step| step.pat.to_string())
            .collect::<Vec<_>>();

        prop_assert_eq!(actual_fixtures, expected_fixtures);
        prop_assert_eq!(actual_steps, expected_steps);
        for (kind, arg) in kinds.iter().map(|kind| arg_kind(*kind)).zip(&args) {
            prop_assert_eq!(arg.as_fixture().is_some(), matches!(kind, ArgKind::Fixture));
            prop_assert_eq!(arg.as_step().is_some(), matches!(kind, ArgKind::Step));
        }
    }

    /// Type-specific test binders preserve each filtered argument's association.
    #[test]
    fn type_specific_binders_preserve_filtered_argument_associations(kinds in prop::collection::vec(0u8..5, 0..32)) {
        let args = arguments(&kinds);
        let fixture_args = args
            .iter()
            .filter(|arg| arg.as_fixture().is_some())
            .collect::<Vec<_>>();
        let step_args = args
            .iter()
            .filter(|arg| arg.as_step().is_some())
            .collect::<Vec<_>>();
        let fixture_names = fixture_names(&args);
        let step_names = step_names(&args);
        let fixture_bindings = bindings("fixture_binding", fixture_args.len());
        let step_bindings = bindings("step_binding", step_args.len());

        let bound_fixtures = bind_fixture_args(&fixture_args, &fixture_bindings);
        let bound_steps = bind_args(&step_args, &step_bindings);

        assert_fixture_bindings(&bound_fixtures, &fixture_names);
        assert_step_bindings(&bound_steps, &step_names);
    }
}
