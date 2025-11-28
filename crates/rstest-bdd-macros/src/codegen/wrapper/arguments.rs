//! Argument code generation utilities shared by wrapper emission logic.

use super::args::{Arg, DataTableArg, DocStringArg, StepStructArg};
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};

mod fixtures;
use fixtures::gen_fixture_decls;

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

fn gen_cache_key_struct() -> TokenStream2 {
    quote! {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        struct __rstest_bdd_table_key {
            hash: u64,
            rows: usize,
        }

        impl __rstest_bdd_table_key {
            fn new(table: &[&[&str]]) -> Self {
                use std::hash::{Hash, Hasher};
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                table.len().hash(&mut hasher);
                for row in table {
                    row.len().hash(&mut hasher);
                    for cell in *row {
                        cell.hash(&mut hasher);
                    }
                }
                Self {
                    hash: hasher.finish(),
                    rows: table.len(),
                }
            }
        }
    }
}

fn gen_cache_static() -> TokenStream2 {
    quote! {
        static __RSTEST_BDD_TABLE_CACHE: std::sync::OnceLock<
            std::sync::Mutex<
                std::collections::HashMap<
                    __rstest_bdd_table_key,
                    std::sync::Arc<Vec<Vec<String>>>,
                >,
            >,
        > = std::sync::OnceLock::new();
    }
}

fn gen_table_lookup(path: &TokenStream2, convert_err: &TokenStream2) -> TokenStream2 {
    quote! {
        let cache = __RSTEST_BDD_TABLE_CACHE.get_or_init(|| {
            std::sync::Mutex::new(std::collections::HashMap::new())
        });
        let arc_table = {
            let mut guard = cache.lock().map_err(|e| #convert_err)?;
            guard
                .entry(key)
                .or_insert_with(|| {
                    std::sync::Arc::new(
                        table
                            .iter()
                            .map(|row| {
                                row.iter()
                                    .map(|cell| cell.to_string())
                                    .collect::<Vec<String>>()
                            })
                            .collect::<Vec<Vec<String>>>(),
                    )
                })
                .clone()
        };
        let cached_table = #path::datatable::CachedTable::from_arc(arc_table);
    }
}

fn is_cached_table_type(declared_ty: &syn::Type) -> bool {
    matches!(
        declared_ty,
        syn::Type::Path(ref ty_path)
            if ty_path
                .path
                .segments
                .last()
                .is_some_and(|s| s.ident == "CachedTable")
    )
}

/// Generate declaration for a data table argument.
pub(super) fn gen_datatable_decl(
    datatable: Option<DataTableArg<'_>>,
    pattern: &syn::LitStr,
    ident: &syn::Ident,
) -> Option<TokenStream2> {
    datatable.map(|arg| {
        let pat = arg.pat.clone();
        let declared_ty = arg.ty.clone();
        let ty = quote! { #declared_ty };
        let is_cached_table = is_cached_table_type(&declared_ty);
        let path = crate::codegen::rstest_bdd_path();
        let StepMeta { pattern, ident } = StepMeta { pattern, ident };
        let missing_err = step_error_tokens(
            &format_ident!("ExecutionError"),
            pattern,
            ident,
            &quote! { format!("Step '{}' requires a data table", #pattern) },
        );
        let convert_err = step_error_tokens(
            &format_ident!("ExecutionError"),
            pattern,
            ident,
            &quote! { format!("failed to convert auxiliary argument for step '{}': {}", #pattern, e) },
        );
        let conversion = if is_cached_table {
            quote! { cached_table }
        } else {
            quote! {
                {
                    let owned: Vec<Vec<String>> = cached_table.into();
                    owned
                }
                .try_into()
                .map_err(|e| #convert_err)?
            }
        };
        let cache_key_struct = gen_cache_key_struct();
        let cache_static = gen_cache_static();
        let table_lookup = gen_table_lookup(&path, &convert_err);

        quote! {
            #cache_key_struct
            #cache_static

            let #pat: #ty = {
                let table = _table.ok_or_else(|| #missing_err)?;
                let key = __rstest_bdd_table_key::new(table);
                #table_lookup
                #conversion
            };
        }
    })
}

/// Generate declaration for a doc string argument.
///
/// Step functions require an owned `String`, so the wrapper copies the block.
pub(super) fn gen_docstring_decl(
    docstring: Option<DocStringArg<'_>>,
    pattern: &syn::LitStr,
    ident: &syn::Ident,
) -> Option<TokenStream2> {
    gen_optional_decl(
        docstring,
        StepMeta { pattern, ident },
        "requires a doc string",
        |arg: DocStringArg<'_>| {
            let pat = arg.pat.clone();
            let ty = quote! { String };
            let expr = quote! { _docstring.map(|s| s.to_owned()) };
            (pat, ty, expr)
        },
    )
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

fn generate_capture_initializers(
    captures: &[TokenStream2],
    missing_errs: &[TokenStream2],
    values_ident: &proc_macro2::Ident,
) -> Vec<TokenStream2> {
    captures
        .iter()
        .zip(missing_errs.iter())
        .map(|(capture, missing)| {
            quote! {
                let raw = #capture.ok_or_else(|| #missing)?;
                #values_ident.push(raw.to_string());
            }
        })
        .collect()
}

fn gen_step_struct_decl(
    step_struct: Option<StepStructArg<'_>>,
    captures: &[TokenStream2],
    placeholder_names: &[syn::LitStr],
    meta: StepMeta<'_>,
) -> Option<TokenStream2> {
    let capture_count = placeholder_names.len();
    step_struct.map(|arg| {
        let StepStructArg { pat, ty } = arg;
        let values_ident = format_ident!("__rstest_bdd_struct_values");
        let StepMeta { pattern, ident } = meta;
        let missing_errs = generate_missing_capture_errors(placeholder_names, pattern, ident, pat);
        let capture_inits = generate_capture_initializers(captures, &missing_errs, &values_ident);
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
            let #pat: #ty = ::std::convert::TryFrom::try_from(#values_ident)
                .map_err(|error| #convert_err)?;
        }
    })
}

/// Generate code to parse step arguments from regex captures.
pub(super) fn gen_step_parses(
    step_args: &[&Arg],
    captured: &[TokenStream2],
    meta: StepMeta<'_>,
) -> Vec<TokenStream2> {
    let StepMeta { pattern, ident } = meta;
    step_args
        .iter()
        .zip(captured.iter().enumerate())
        .map(|(arg, (idx, capture))| {
            let Arg::Step { pat, ty } = *arg else {
                unreachable!("step argument vector must contain step args");
            };
            let raw_ident = format_ident!("__raw{}", idx);
            let missing_cap_err = step_error_tokens(
                &format_ident!("ExecutionError"),
                pattern,
                ident,
                &quote! {
                    format!(
                        "pattern '{}' missing capture for argument '{}'",
                        #pattern,
                        stringify!(#pat),
                    )
                },
            );
            let parse_err = step_error_tokens(
                &format_ident!("ExecutionError"),
                pattern,
                ident,
                &quote! {
                    format!(
                        "failed to parse argument '{}' of type '{}' from pattern '{}' with captured value: '{:?}'",
                        stringify!(#pat),
                        stringify!(#ty),
                        #pattern,
                        #raw_ident,
                    )
                },
            );
            quote! {
                let #raw_ident = #capture.ok_or_else(|| #missing_cap_err)?;
                let #pat: #ty = (#raw_ident).parse().map_err(|_| #parse_err)?;
            }
        })
        .collect()
}

/// Generate declarations and parsing logic for wrapper arguments.
pub(super) fn prepare_argument_processing(
    args: &[Arg],
    step_meta: StepMeta<'_>,
    ctx_ident: &proc_macro2::Ident,
    placeholder_names: &[syn::LitStr],
) -> PreparedArgs {
    let StepMeta { pattern, ident } = step_meta;
    let mut fixtures = Vec::new();
    let mut step_args = Vec::new();
    let mut step_struct: Option<&Arg> = None;
    let mut datatable: Option<&Arg> = None;
    let mut docstring: Option<&Arg> = None;

    for arg in args {
        match arg {
            Arg::Fixture { .. } => fixtures.push(arg),
            Arg::Step { .. } => step_args.push(arg),
            Arg::StepStruct { .. } => step_struct = Some(arg),
            Arg::DataTable { .. } => datatable = Some(arg),
            Arg::DocString { .. } => docstring = Some(arg),
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
        gen_step_parses(&step_args, capture_slice, step_meta)
    };
    let step_struct_decl = gen_step_struct_decl(
        step_struct.and_then(Arg::as_step_struct),
        &all_captures,
        placeholder_names,
        step_meta,
    );
    let datatable_decl = gen_datatable_decl(datatable.and_then(Arg::as_datatable), pattern, ident);
    let docstring_decl = gen_docstring_decl(docstring.and_then(Arg::as_docstring), pattern, ident);
    PreparedArgs {
        declares,
        step_arg_parses,
        step_struct_decl,
        datatable_decl,
        docstring_decl,
    }
}

/// Collect argument identifiers in the order declared by the step function.
pub(super) fn collect_ordered_arguments(args: &[Arg]) -> Vec<&syn::Ident> {
    args.iter().map(Arg::pat).collect()
}

#[cfg(test)]
mod tests;
