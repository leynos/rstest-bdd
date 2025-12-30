//! Argument code generation utilities shared by wrapper emission logic.

use super::args::{Arg, DataTableArg, DocStringArg, StepStructArg};
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};

mod datatable;
mod fixtures;
mod step_parse;
use datatable::{CacheIdents, gen_datatable_decl};
use fixtures::gen_fixture_decls;
use step_parse::{ArgParseContext, gen_quote_strip_to_stripped, gen_single_step_parse};

#[derive(Copy, Clone)]
pub(super) struct StepMeta<'a> {
    pub(super) pattern: &'a syn::LitStr,
    pub(super) ident: &'a syn::Ident,
}

pub(super) struct PreparedArgs {
    pub(super) declares: Vec<TokenStream2>,
    pub(super) step_arg_parses: Vec<TokenStream2>,
    pub(super) step_struct_decl: Option<TokenStream2>,
    pub(super) datatable_decl: Option<TokenStream2>,
    pub(super) docstring_decl: Option<TokenStream2>,
}

/// Wrapper-local argument bindings avoid leading underscores to keep Clippy happy.
#[derive(Copy, Clone)]
struct BoundArg<'a> {
    arg: &'a Arg,
    binding: &'a syn::Ident,
}

#[derive(Copy, Clone)]
struct BoundStepStructArg<'a> {
    arg: StepStructArg<'a>,
    binding: &'a syn::Ident,
}

#[derive(Copy, Clone)]
struct BoundDataTableArg<'a> {
    arg: DataTableArg<'a>,
    binding: &'a syn::Ident,
}

#[derive(Copy, Clone)]
struct BoundDocStringArg<'a> {
    arg: DocStringArg<'a>,
    binding: &'a syn::Ident,
}

fn wrapper_binding_ident(index: usize) -> syn::Ident {
    format_ident!("rstest_bdd_arg_{index}")
}

fn wrapper_binding_idents(args: &[Arg]) -> Vec<syn::Ident> {
    args.iter()
        .enumerate()
        .map(|(idx, _)| wrapper_binding_ident(idx))
        .collect()
}

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
            let expr = quote! { _docstring.map(|s| s.to_owned()) };
            (pat, ty, expr)
        },
    )
}

/// Context for generating capture initialisers in struct-based step arguments.
///
/// Groups the parameters required by [`generate_capture_initializers`] to reduce
/// the function's argument count.
struct CaptureInitContext<'a> {
    captures: &'a [TokenStream2],
    missing_errs: &'a [TokenStream2],
    hints: &'a [Option<String>],
    values_ident: &'a proc_macro2::Ident,
    meta: StepMeta<'a>,
    struct_pat: &'a syn::Ident,
}

fn generate_missing_capture_errors(
    placeholder_names: &[syn::LitStr],
    pattern: &syn::LitStr,
    ident: &syn::Ident,
    pat: &syn::Ident,
) -> Vec<TokenStream2> {
    placeholder_names
        .iter()
        .map(|name| {
            step_error_tokens(
                &format_ident!("ExecutionError"),
                pattern,
                ident,
                &quote! {
                    format!(
                        "pattern '{}' missing capture for placeholder '{{{}}}' required by '{}'",
                        #pattern,
                        #name,
                        stringify!(#pat),
                    )
                },
            )
        })
        .collect()
}

fn generate_capture_initializers(ctx: &CaptureInitContext<'_>) -> Vec<TokenStream2> {
    let CaptureInitContext {
        captures,
        missing_errs,
        hints,
        values_ident,
        meta,
        struct_pat,
    } = ctx;
    let StepMeta { pattern, ident } = meta;
    let raw_ident = format_ident!("raw");
    captures
        .iter()
        .zip(missing_errs.iter())
        .enumerate()
        .map(|(idx, (capture, missing))| {
            let hint = hints.get(idx).and_then(|h| h.as_deref());
            let needs_quote_strip = rstest_bdd_patterns::requires_quote_stripping(hint);
            if needs_quote_strip {
                let malformed_err = step_error_tokens(
                    &format_ident!("ExecutionError"),
                    pattern,
                    ident,
                    &quote! {
                        format!(
                            "malformed quoted string for '{}' capture {}: expected at least 2 characters, got '{}'",
                            stringify!(#struct_pat),
                            #idx,
                            #raw_ident,
                        )
                    },
                );
                let quote_strip = gen_quote_strip_to_stripped(&raw_ident, &malformed_err);
                quote! {
                    let #raw_ident = #capture.ok_or_else(|| #missing)?;
                    #quote_strip
                    #values_ident.push(stripped.to_string());
                }
            } else {
                quote! {
                    let #raw_ident = #capture.ok_or_else(|| #missing)?;
                    #values_ident.push(#raw_ident.to_string());
                }
            }
        })
        .collect()
}

/// Placeholder information needed for step struct code generation.
struct PlaceholderInfo<'a> {
    captures: &'a [TokenStream2],
    names: &'a [syn::LitStr],
    hints: &'a [Option<String>],
}

fn gen_step_struct_decl(
    step_struct: Option<BoundStepStructArg<'_>>,
    placeholders: &PlaceholderInfo<'_>,
    meta: StepMeta<'_>,
) -> Option<TokenStream2> {
    let PlaceholderInfo {
        captures,
        names,
        hints,
    } = placeholders;
    let capture_count = names.len();
    step_struct.map(|arg| {
        let StepStructArg { pat, ty } = arg.arg;
        let binding = arg.binding;
        let values_ident = format_ident!("__rstest_bdd_struct_values");
        let StepMeta { pattern, ident } = meta;
        let missing_errs = generate_missing_capture_errors(names, pattern, ident, pat);
        let capture_init_ctx = CaptureInitContext {
            captures,
            missing_errs: &missing_errs,
            hints,
            values_ident: &values_ident,
            meta,
            struct_pat: pat,
        };
        let capture_inits = generate_capture_initializers(&capture_init_ctx);
        let convert_err = step_error_tokens(
            &format_ident!("ExecutionError"),
            pattern,
            ident,
            &quote! {
                format!(
                    "failed to populate '{}' from pattern '{}': {}",
                    stringify!(#pat),
                    #pattern,
                    error
                )
            },
        );
        quote! {
            let mut #values_ident = Vec::with_capacity(#capture_count);
            #(#capture_inits)*
            let #binding: #ty = ::std::convert::TryFrom::try_from(#values_ident)
                .map_err(|error| #convert_err)?;
        }
    })
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
    step_args: &[BoundArg<'_>],
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

/// Generate declarations and parsing logic for wrapper arguments.
pub(super) fn prepare_argument_processing(
    args: &[Arg],
    step_meta: StepMeta<'_>,
    ctx_ident: &proc_macro2::Ident,
    placeholder_names: &[syn::LitStr],
    placeholder_hints: &[Option<String>],
    datatable_idents: Option<(&proc_macro2::Ident, &proc_macro2::Ident)>,
) -> PreparedArgs {
    let StepMeta { pattern, ident } = step_meta;
    let binding_idents = wrapper_binding_idents(args);
    let mut fixtures = Vec::new();
    let mut step_args = Vec::new();
    let mut step_struct: Option<BoundStepStructArg<'_>> = None;
    let mut datatable: Option<BoundDataTableArg<'_>> = None;
    let mut docstring: Option<BoundDocStringArg<'_>> = None;

    for (idx, arg) in args.iter().enumerate() {
        let binding = &binding_idents[idx];
        match arg {
            Arg::Fixture { .. } => fixtures.push(BoundArg { arg, binding }),
            Arg::Step { .. } => step_args.push(BoundArg { arg, binding }),
            Arg::StepStruct { pat, ty } => {
                step_struct = Some(BoundStepStructArg {
                    arg: StepStructArg { pat, ty },
                    binding,
                })
            }
            Arg::DataTable { pat, ty } => {
                datatable = Some(BoundDataTableArg {
                    arg: DataTableArg { pat, ty },
                    binding,
                })
            }
            Arg::DocString { pat } => {
                docstring = Some(BoundDocStringArg {
                    arg: DocStringArg { pat },
                    binding,
                })
            }
        }
    }

    let declares = gen_fixture_decls(&fixtures, ident, ctx_ident);
    let all_captures: Vec<_> = placeholder_names
        .iter()
        .enumerate()
        .map(|(idx, _)| {
            let index = syn::Index::from(idx);
            quote! { captures.get(#index).map(|m| m.as_str()) }
        })
        .collect();
    let step_arg_parses = if step_struct.is_some() {
        Vec::new()
    } else {
        let capture_slice = all_captures.get(..step_args.len()).unwrap_or_else(|| {
            panic!(
                "step arguments ({}) cannot exceed capture count ({})",
                step_args.len(),
                all_captures.len()
            )
        });
        let hint_slice = placeholder_hints.get(..step_args.len()).unwrap_or_else(|| {
            panic!(
                "placeholder hints ({}) must match or exceed step argument count ({})",
                placeholder_hints.len(),
                step_args.len()
            )
        });
        gen_step_parses(&step_args, capture_slice, hint_slice, step_meta)
    };
    let step_struct_decl = gen_step_struct_decl(
        step_struct,
        &PlaceholderInfo {
            captures: &all_captures,
            names: placeholder_names,
            hints: placeholder_hints,
        },
        step_meta,
    );
    let datatable_decl = match (datatable, datatable_idents) {
        (Some(dt), Some((key_ident, cache_ident))) => {
            let cache_idents = CacheIdents {
                key: key_ident,
                cache: cache_ident,
            };
            gen_datatable_decl(Some(dt), step_meta, &cache_idents)
        }
        _ => None,
    };
    let docstring_decl = gen_docstring_decl(docstring, pattern, ident);
    PreparedArgs {
        declares,
        step_arg_parses,
        step_struct_decl,
        datatable_decl,
        docstring_decl,
    }
}

/// Collect wrapper-local argument bindings in the order declared by the step function.
pub(super) fn collect_ordered_arguments(args: &[Arg]) -> Vec<syn::Ident> {
    wrapper_binding_idents(args)
}

#[cfg(test)]
mod tests;
