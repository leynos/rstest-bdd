//! Attribute macro implementations.
//!
//! This module is the entry layer for the crate's attribute macros. It declares
//! the per-macro modules and re-exports their entry points (`given`, `when`,
//! `then`, `scenario`, and `scenarios`) for `lib.rs` to hang the
//! `#[proc_macro_attribute]` shims on.
//!
//! It also owns the machinery the three step attributes share, because
//! `#[given]`, `#[when]`, and `#[then]` differ only by their `StepKeyword`:
//!
//! - `StepAttrArgs` parses the attribute arguments — an optional pattern literal, the `expr =
//!   "..."` cucumber-rs spelling, and the `result` / `value` return-kind hint.
//! - `determine_step_pattern` falls back to inferring a pattern from the function name when none is
//!   supplied.
//! - `extract_step_args_or_abort` and `signature_error_help` turn a signature rejection into a
//!   keyword-specific diagnostic with accurate spans.
//! - `step_attr` drives that sequence and `inject_skip_scope` wraps the body so the runtime can
//!   validate `skip!` against the enclosing scope.
//!
//! Note the direction of delegation: `given`, `when`, and `then` are one-line
//! shims that call `step_attr` with their keyword, rather than owning an
//! expansion of their own. The `scenario` and `scenarios` sub-modules do own
//! theirs. Token generation itself belongs to the `codegen` module; parsing
//! helpers live in `utils` and return-type classification in
//! `return_classifier`.

use proc_macro::TokenStream;
use quote::quote;

mod given;
mod scenario;
pub(crate) mod scenarios;
mod skip_scope;
mod step_attr_args;
mod then;
mod when;

pub(crate) use given::given;
pub(crate) use scenario::scenario;
pub(crate) use scenarios::scenarios;
use skip_scope::inject_skip_scope;
use step_attr_args::StepAttrArgs;
pub(crate) use then::then;
pub(crate) use when::when;

use crate::{
    codegen::wrapper::{WrapperConfig, args::ExtractedArgs, extract_args, generate_wrapper_code},
    return_classifier::{StepReturnStrategy, classify_step_return_type},
    utils::{
        errors::error_to_tokens,
        pattern::{infer_pattern, placeholder_names},
    },
};

/// Determine the step pattern literal for a step function.
///
/// When no pattern is provided (or only whitespace is provided), the pattern is
/// inferred from the step function name. An explicit empty string literal is
/// preserved and registers an empty pattern.
fn determine_step_pattern(pattern: Option<syn::LitStr>, ident: &syn::Ident) -> syn::LitStr {
    pattern.map_or_else(
        || infer_pattern(ident),
        |lit| {
            let value = lit.value();
            if value.is_empty() {
                lit
            } else if value.trim().is_empty() {
                infer_pattern(ident)
            } else {
                lit
            }
        },
    )
}

/// Extract step arguments from the function signature or abort macro expansion.
///
/// This centralizes argument extraction so we can provide keyword-specific
/// diagnostics and help text while preserving accurate spans.
fn extract_step_args_or_abort(
    func: &mut syn::ItemFn,
    unique_placeholders: &mut std::collections::HashSet<String>,
    keyword: crate::StepKeyword,
) -> ExtractedArgs {
    match extract_args(func, unique_placeholders) {
        Ok(args) => args,
        Err(err) => {
            let err_message = err.to_string();
            let help = signature_error_help(&err_message, keyword);
            if err_message.contains("unsupported parameter pattern") {
                if let Some(pattern) = first_non_identifier_pattern(func) {
                    proc_macro_error3::abort!(
                        pattern,
                        "invalid step function signature: {}",
                        err;
                        help = help
                    );
                }
            }
            proc_macro_error3::abort!(
                err.span(),
                "invalid step function signature: {}",
                err;
                help = help
            );
        }
    }
}

/// Find the first function parameter pattern that is not a simple identifier.
fn first_non_identifier_pattern(func: &syn::ItemFn) -> Option<&syn::Pat> {
    func.sig.inputs.iter().find_map(|arg| match arg {
        syn::FnArg::Typed(pat_ty) => match &*pat_ty.pat {
            syn::Pat::Ident(_) => None,
            other => Some(other),
        },
        syn::FnArg::Receiver(_) => None,
    })
}

/// Return the lowercase attribute name for a [`StepKeyword`].
fn keyword_name(keyword: crate::StepKeyword) -> &'static str {
    match keyword {
        crate::StepKeyword::Given => "given",
        crate::StepKeyword::When => "when",
        crate::StepKeyword::Then => "then",
        crate::StepKeyword::And => "and",
        crate::StepKeyword::But => "but",
    }
}

/// Produce a keyword-specific help message for a step signature diagnostic.
fn signature_error_help(err_message: &str, keyword: crate::StepKeyword) -> String {
    if err_message.contains("duplicate `#[datatable]` attribute") {
        return "Remove one of the duplicate `#[datatable]` attributes.".to_owned();
    }

    if err_message.contains("duplicate `#[from]` attribute") {
        return "Remove one of the duplicate `#[from]` attributes.".to_owned();
    }

    if err_message.contains(crate::codegen::wrapper::args::classify::DUPLICATE_DATATABLE_ERROR) {
        return "Remove one of the DataTable parameters.".to_owned();
    }

    if err_message.contains("unsupported parameter pattern") {
        return concat!(
            "Bind the parameter to a simple identifier (e.g., `tuple: (i32, i32)` or `user: \
             User`) ",
            "and destructure it inside the step body."
        )
        .to_owned();
    }

    if err_message.contains("methods are not supported; remove `self`") {
        return "Remove `self` from step functions.".to_owned();
    }

    let kw_name = keyword_name(keyword);
    format!(
        "Use a step attribute (such as `#[{kw_name}]`) on `fn name(...args...)` with supported \
         step arguments/fixtures (step attributes include `#[given]`, `#[when]`, and `#[then]`); \
         remove `self` if present."
    )
}

/// Inputs used to generate wrapper code for a step function.
struct WrapperInputs<'a> {
    /// Stores the internal `func` value.
    func: &'a syn::ItemFn,
    /// Stores the internal `pattern` value.
    pattern: &'a syn::LitStr,
    /// Stores the internal `keyword` value.
    keyword: crate::StepKeyword,
    /// Stores the internal `args` value.
    args: &'a ExtractedArgs,
    /// Stores the internal `placeholder_names` value.
    placeholder_names: &'a [syn::LitStr],
    /// Stores the internal `placeholder_hints` value.
    placeholder_hints: &'a [Option<String>],
    /// Stores the internal `strategy` value.
    strategy: StepReturnStrategy,
}

/// Build wrapper configuration from [`WrapperInputs`] and emit the wrapper tokens.
fn build_and_generate_wrapper(inputs: &WrapperInputs<'_>) -> proc_macro2::TokenStream {
    let config = WrapperConfig {
        ident: &inputs.func.sig.ident,
        is_async_step: inputs.func.sig.asyncness.is_some(),
        args: inputs.args,
        pattern: inputs.pattern,
        keyword: inputs.keyword,
        placeholder_names: inputs.placeholder_names,
        placeholder_hints: inputs.placeholder_hints,
        capture_count: inputs.placeholder_names.len(),
        strategy: inputs.strategy,
    };
    generate_wrapper_code(&config)
}

/// Core implementation for step attribute macros.
///
/// Parses the attribute arguments, determines the step pattern, extracts and
/// classifies function arguments, computes the return kind, and generates the
/// wrapper code. Emits the original function alongside the generated wrapper.
fn step_attr(attr: TokenStream, item: TokenStream, keyword: crate::StepKeyword) -> TokenStream {
    let mut func = syn::parse_macro_input!(item as syn::ItemFn);
    #[cfg(feature = "compile-time-validation")]
    let library = take_step_library_attribute(&mut func.attrs);
    #[cfg(not(feature = "compile-time-validation"))]
    take_step_library_attribute(&mut func.attrs);
    inject_skip_scope(&mut func);
    let attr_args = if attr.is_empty() {
        StepAttrArgs {
            pattern: None,
            return_override: None,
        }
    } else {
        syn::parse_macro_input!(attr as StepAttrArgs)
    };
    let pattern = determine_step_pattern(attr_args.pattern, &func.sig.ident);
    #[cfg(feature = "compile-time-validation")]
    if let Some(library) = library.as_deref() {
        crate::validation::steps::register_step_in_library(keyword, &pattern, library);
    } else {
        crate::validation::steps::register_step(keyword, &pattern);
    }
    let mut placeholder_summary = match placeholder_names(&pattern.value()) {
        Ok(set) => set,
        Err(mut err) => {
            // Anchor diagnostics on the attribute literal for clarity.
            err.combine(syn::Error::new(pattern.span(), "in this step pattern"));
            return error_to_tokens(&err).into();
        }
    };

    let args = extract_step_args_or_abort(&mut func, &mut placeholder_summary.unique, keyword);

    let placeholder_literals: Vec<_> = placeholder_summary
        .ordered
        .iter()
        .map(|info| syn::LitStr::new(&info.name, pattern.span()))
        .collect();
    let placeholder_hints: Vec<_> = placeholder_summary
        .ordered
        .iter()
        .map(|info| info.hint.clone())
        .collect();
    let strategy = match classify_step_return_type(&func.sig.output, attr_args.return_override) {
        Ok(kind) => kind,
        Err(err) => return error_to_tokens(&err).into(),
    };

    let wrapper_code = build_and_generate_wrapper(&WrapperInputs {
        func: &func,
        pattern: &pattern,
        keyword,
        args: &args,
        placeholder_names: &placeholder_literals,
        placeholder_hints: &placeholder_hints,
        strategy,
    });

    TokenStream::from(quote! {
        #func
        #wrapper_code
    })
}

/// Remove and return the lexical-library marker injected by `#[step_library]`.
fn take_step_library_attribute(attributes: &mut Vec<syn::Attribute>) -> Option<String> {
    let index = attributes.iter().position(|attribute| {
        attribute
            .path()
            .is_ident("rstest_bdd_internal_step_library")
    })?;
    let attribute = attributes.remove(index);
    let syn::Meta::NameValue(value) = attribute.meta else {
        return None;
    };
    let syn::Expr::Lit(expression) = value.value else {
        return None;
    };
    let syn::Lit::Str(library) = expression.lit else {
        return None;
    };
    Some(library.value())
}
#[cfg(test)]
mod tests;
