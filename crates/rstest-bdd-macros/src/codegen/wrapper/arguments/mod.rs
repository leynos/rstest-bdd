//! Argument code generation utilities shared by wrapper emission logic.

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use rstest_bdd_patterns::requires_quote_stripping;

use super::args::{Arg, DataTableArg, FixtureArg, StepArg, StepStructArg};

mod bindings;
mod declarations;

mod datatable;
mod fixtures;
mod step_parse;
mod step_struct;

use bindings::{
    BoundDataTableArg,
    BoundDocStringArg,
    BoundFixtureArg,
    BoundStepArg,
    BoundStepStructArg,
    wrapper_binding_idents,
};
use datatable::{CacheIdents, gen_datatable_decl};
pub(super) use declarations::{PreparedArgs, StepMeta};
use fixtures::gen_fixture_decls;
use step_parse::{ArgParseContext, gen_single_step_parse};
use step_struct::{PlaceholderInfo, gen_step_struct_decl};

/// Check if a type is a reference to str (i.e., `&str` or `&'a str`).
///
/// This function examines the type structure to determine if it represents
/// a borrowed string slice. It handles both simple `&str` and lifetime-annotated
/// variants like `&'a str`. Mutable references (`&mut str`) are not considered
/// valid for step arguments since captured values are immutable.
fn is_str_reference(ty: &syn::Type) -> bool {
    if let syn::Type::Reference(type_ref) = ty {
        if type_ref.mutability.is_some() {
            return false;
        }
        matches!(
            &*type_ref.elem,
            syn::Type::Path(path) if path.qself.is_none() && path.path.is_ident("str")
        )
    } else {
        false
    }
}

/// Quote construction for [`StepError`] variants sharing `pattern`,
/// `function` and `message` fields.
pub(super) fn step_error_tokens(
    variant: &syn::Ident,
    pattern: &syn::LitStr,
    ident: &syn::Ident,
    message: &TokenStream2,
) -> TokenStream2 {
    let path = crate::codegen::rstest_bdd_path();
    quote! {
        #path::StepError::#variant {
            pattern: #pattern.to_string(),
            function: stringify!(#ident).to_string(),
            message: #message,
        }
    }
}

/// Provides the internal `gen_optional_decl` operation.
fn gen_optional_decl<T, F>(
    arg: Option<T>,
    meta: StepMeta<'_>,
    error_msg: &str,
    generator: F,
) -> Option<TokenStream2>
where
    F: FnOnce(T) -> (syn::Ident, TokenStream2, TokenStream2),
{
    arg.map(|arg_value| {
        let (pat, ty, expr) = generator(arg_value);
        let StepMeta { pattern, ident } = meta;
        let missing_err = step_error_tokens(
            &format_ident!("ExecutionError"),
            pattern,
            ident,
            &quote! { format!("Step '{}' {}", #pattern, #error_msg) },
        );
        let convert_err = step_error_tokens(
            &format_ident!("ExecutionError"),
            pattern,
            ident,
            &quote! { format!("failed to convert auxiliary argument for step '{}': {}", #pattern, e) },
        );
        quote! {
            let #pat: #ty = #expr
                .ok_or_else(|| #missing_err)?
                .try_into()
                .map_err(|e| #convert_err)?;
        }
    })
}

/// Generate declaration for a doc string argument.
///
/// Step functions require an owned `String`, so the wrapper copies the block.
pub(super) fn gen_docstring_decl(
    docstring: Option<BoundDocStringArg<'_>>,
    pattern: &syn::LitStr,
    ident: &syn::Ident,
) -> Option<TokenStream2> {
    gen_optional_decl(
        docstring,
        StepMeta { pattern, ident },
        "requires a doc string",
        |arg: BoundDocStringArg<'_>| {
            let pat = arg.binding.clone();
            let ty = quote! { String };
            let expr = quote! { docstring.map(|s| s.to_owned()) };
            (pat, ty, expr)
        },
    )
}

/// Generate code to parse step arguments from regex captures.
///
/// For borrowed `&str` parameters, the captured string slice is used directly
/// without parsing. For all other types, the standard `.parse()` path is used
/// which requires the target type to implement [`FromStr`].
///
/// When a placeholder has the `:string` type hint, the surrounding quotes are
/// stripped from the captured value before assignment or parsing.
pub(super) fn gen_step_parses(
    step_args: &[BoundStepArg<'_>],
    captured: &[TokenStream2],
    hints: &[Option<String>],
    meta: StepMeta<'_>,
) -> Vec<TokenStream2> {
    step_args
        .iter()
        .zip(captured.iter().enumerate())
        .map(|(arg, (idx, capture))| {
            let hint = hints.get(idx).and_then(|h| h.as_deref());
            let ctx = ArgParseContext {
                arg: arg.arg,
                binding: arg.binding,
                idx,
                capture,
                hint,
            };
            gen_single_step_parse(ctx, meta)
        })
        .collect()
}

/// Generated step argument parses and quote-stripping indicator.
struct StepArgParseResult {
    /// Stores the internal `step_arg_parses` value.
    step_arg_parses: Vec<TokenStream2>,
    /// Stores the internal `has_step_arg_quote_strip` value.
    has_step_arg_quote_strip: bool,
}

/// Input data for building step argument parse expressions.
#[derive(Copy, Clone)]
struct StepArgParseInputs<'a> {
    /// Stores the internal `step_args` value.
    step_args: &'a [BoundStepArg<'a>],
    /// Stores the internal `all_captures` value.
    all_captures: &'a [TokenStream2],
    /// Stores the internal `placeholder_hints` value.
    placeholder_hints: &'a [Option<String>],
}

/// Build parsers for step arguments from `StepArgParseInputs` and `StepMeta`.
///
/// When `step_struct_present` is true, the step struct handles argument parsing,
/// so this returns an empty `StepArgParseResult` with no quote-stripping.
fn build_step_arg_parses(
    inputs: StepArgParseInputs<'_>,
    step_meta: StepMeta<'_>,
    step_struct_present: bool,
) -> StepArgParseResult {
    let StepArgParseInputs {
        step_args,
        all_captures,
        placeholder_hints,
    } = inputs;
    if step_struct_present {
        return StepArgParseResult {
            step_arg_parses: Vec::new(),
            has_step_arg_quote_strip: false,
        };
    }

    let Some(capture_slice) = all_captures.get(..step_args.len()) else {
        let error = syn::Error::new(
            step_meta.pattern.span(),
            format!(
                "step arguments ({}) cannot exceed capture count ({})",
                step_args.len(),
                all_captures.len()
            ),
        )
        .to_compile_error();
        return StepArgParseResult {
            step_arg_parses: vec![error],
            has_step_arg_quote_strip: false,
        };
    };
    let Some(hint_slice) = placeholder_hints.get(..step_args.len()) else {
        let error = syn::Error::new(
            step_meta.pattern.span(),
            format!(
                "placeholder hints ({}) must match or exceed step argument count ({})",
                placeholder_hints.len(),
                step_args.len()
            ),
        )
        .to_compile_error();
        return StepArgParseResult {
            step_arg_parses: vec![error],
            has_step_arg_quote_strip: false,
        };
    };
    let has_step_arg_quote_strip = hint_slice
        .iter()
        .any(|hint| requires_quote_stripping(hint.as_deref()));
    let step_arg_parses = gen_step_parses(step_args, capture_slice, hint_slice, step_meta);

    StepArgParseResult {
        step_arg_parses,
        has_step_arg_quote_strip,
    }
}

/// Placeholder metadata and generated identifiers a wrapper needs.
#[derive(Copy, Clone)]
pub(super) struct ArgumentProcessingInputs<'a> {
    /// Identifier bound to the step context inside the wrapper.
    pub(super) ctx_ident: &'a proc_macro2::Ident,
    /// Placeholder names in pattern order.
    pub(super) placeholder_names: &'a [syn::LitStr],
    /// Optional type hints, aligned with `placeholder_names`.
    pub(super) placeholder_hints: &'a [Option<String>],
    /// Key and cache identifiers for the data table, when one is present.
    pub(super) datatable_idents: Option<(&'a proc_macro2::Ident, &'a proc_macro2::Ident)>,
}

/// Wrapper arguments partitioned by kind, each paired with its local binding.
struct BoundArguments<'a> {
    /// Stores the internal `fixtures` value.
    fixtures: Vec<BoundFixtureArg<'a>>,
    /// Stores the internal `step_args` value.
    step_args: Vec<BoundStepArg<'a>>,
    /// Stores the internal `step_struct` value.
    step_struct: Option<BoundStepStructArg<'a>>,
    /// Stores the internal `datatable` value.
    datatable: Option<BoundDataTableArg<'a>>,
    /// Stores the internal `docstring` value.
    docstring: Option<BoundDocStringArg<'a>>,
}

/// Partition extracted arguments by kind, pairing each with its binding.
///
/// `wrapper_binding_idents` mirrors `args`, so a straight zip stays in sync.
fn bind_arguments<'a>(args: &'a [Arg], binding_idents: &'a [syn::Ident]) -> BoundArguments<'a> {
    let mut bound = BoundArguments {
        fixtures: Vec::new(),
        step_args: Vec::new(),
        step_struct: None,
        datatable: None,
        docstring: None,
    };
    for (arg, binding) in args.iter().zip(binding_idents.iter()) {
        match arg {
            Arg::Fixture { name, ty, .. } => bound.fixtures.push(BoundFixtureArg {
                arg: FixtureArg { name, ty },
                binding,
            }),
            Arg::Step { pat, ty } => bound.step_args.push(BoundStepArg {
                arg: StepArg { pat, ty },
                binding,
            }),
            Arg::StepStruct { pat, ty } => {
                bound.step_struct = Some(BoundStepStructArg {
                    arg: StepStructArg { pat, ty },
                    binding,
                });
            }
            Arg::DataTable { ty, .. } => {
                bound.datatable = Some(BoundDataTableArg {
                    arg: DataTableArg { ty },
                    binding,
                });
            }
            Arg::DocString { .. } => {
                bound.docstring = Some(BoundDocStringArg { binding });
            }
        }
    }
    bound
}

/// Build the capture accessors used by generated step-argument parsers.
fn capture_accessors(count: usize) -> Vec<TokenStream2> {
    (0..count)
        .map(|idx| {
            let index = syn::Index::from(idx);
            quote! { captures.get(#index).map(|m| m.as_str()) }
        })
        .collect()
}

/// Generate the data table declaration when both the argument and the cache
/// identifiers are present.
fn datatable_declaration(
    datatable: Option<BoundDataTableArg<'_>>,
    datatable_idents: Option<(&proc_macro2::Ident, &proc_macro2::Ident)>,
    step_meta: StepMeta<'_>,
) -> Option<TokenStream2> {
    let (arg, (key, cache)) = (datatable?, datatable_idents?);
    gen_datatable_decl(Some(arg), step_meta, &CacheIdents { key, cache })
}

/// Generate declarations and parsing logic for wrapper arguments.
pub(super) fn prepare_argument_processing(
    args: &[Arg],
    step_meta: StepMeta<'_>,
    inputs: ArgumentProcessingInputs<'_>,
) -> PreparedArgs {
    let ArgumentProcessingInputs {
        ctx_ident,
        placeholder_names,
        placeholder_hints,
        datatable_idents,
    } = inputs;
    let StepMeta { pattern, ident } = step_meta;
    let binding_idents = wrapper_binding_idents(args);
    debug_assert_eq!(
        binding_idents.len(),
        args.len(),
        "expected one wrapper binding per argument"
    );
    let BoundArguments {
        fixtures,
        step_args,
        step_struct,
        datatable,
        docstring,
    } = bind_arguments(args, &binding_idents);

    let declares = gen_fixture_decls(&fixtures, ident, ctx_ident);
    let all_captures = capture_accessors(placeholder_names.len());
    let step_struct_present = step_struct.is_some();
    let StepArgParseResult {
        step_arg_parses,
        has_step_arg_quote_strip,
    } = build_step_arg_parses(
        StepArgParseInputs {
            step_args: &step_args,
            all_captures: &all_captures,
            placeholder_hints,
        },
        step_meta,
        step_struct_present,
    );
    let step_struct_decl = gen_step_struct_decl(
        step_struct,
        &PlaceholderInfo {
            captures: &all_captures,
            names: placeholder_names,
            hints: placeholder_hints,
        },
        step_meta,
    );
    let datatable_decl = datatable_declaration(datatable, datatable_idents, step_meta);
    let docstring_decl = gen_docstring_decl(docstring, pattern, ident);
    PreparedArgs {
        declares,
        step_arg_parses,
        step_struct_decl,
        datatable_decl,
        docstring_decl,
        expect_lints: Vec::new(),
        has_step_arg_quote_strip,
    }
}

/// Collect wrapper-local argument bindings in the order declared by the step function.
pub(super) fn collect_ordered_arguments(args: &[Arg]) -> Vec<syn::Ident> {
    wrapper_binding_idents(args)
}

#[cfg(test)]
mod tests;
