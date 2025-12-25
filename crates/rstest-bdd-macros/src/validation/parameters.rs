//! Validation for function parameters against scenario outline headers.
//!
//! Underscore-prefixed parameter names (e.g., `_param`) match unprefixed headers
//! (e.g., `param`), enabling idiomatic Rust unused parameter marking.

use crate::utils::errors::error_to_tokens;
use crate::utils::pattern::normalize_param_name;
use proc_macro2::TokenStream;

fn parameter_matches_header(arg: &syn::FnArg, header: &str) -> bool {
    match arg {
        syn::FnArg::Typed(p) => match &*p.pat {
            syn::Pat::Ident(id) => {
                let param_name = id.ident.to_string();
                normalize_param_name(&param_name) == header
            }
            _ => false,
        },
        syn::FnArg::Receiver(_) => false,
    }
}

fn find_matching_parameter<'a>(
    sig: &'a mut syn::Signature,
    header: &str,
) -> Result<&'a mut syn::FnArg, TokenStream> {
    if let Some(pos) = sig
        .inputs
        .iter()
        .position(|arg| parameter_matches_header(arg, header))
    {
        sig.inputs.iter_mut().nth(pos).ok_or_else(|| {
            error_to_tokens(&syn::Error::new(
                proc_macro2::Span::call_site(),
                "position from earlier search exists",
            ))
        })
    } else {
        Err(create_parameter_mismatch_error(sig, header))
    }
}

fn add_case_attribute_if_missing(arg: &mut syn::FnArg) {
    if let syn::FnArg::Typed(p) = arg {
        if !has_case_attribute(p) {
            p.attrs.push(syn::parse_quote!(#[case]));
        }
    }
}

fn has_case_attribute(p: &syn::PatType) -> bool {
    p.attrs.iter().any(|attr| {
        let segs: Vec<_> = attr
            .path()
            .segments
            .iter()
            .map(|s| s.ident.to_string())
            .collect();
        segs == ["case"] || segs == ["rstest", "case"]
    })
}

fn create_parameter_mismatch_error(sig: &syn::Signature, header: &str) -> TokenStream {
    let available_params: Vec<String> = sig
        .inputs
        .iter()
        .filter_map(|arg| match arg {
            syn::FnArg::Typed(p) => match &*p.pat {
                syn::Pat::Ident(id) => Some(id.ident.to_string()),
                _ => None,
            },
            syn::FnArg::Receiver(_) => None,
        })
        .collect();
    let msg = format!(
        "parameter `{header}` not found for scenario outline column. Available parameters: [{}]",
        available_params.join(", ")
    );
    error_to_tokens(&syn::Error::new_spanned(sig, msg))
}

/// Process scenario outline examples and modify function parameters.
pub(crate) fn process_scenario_outline_examples(
    sig: &mut syn::Signature,
    examples: Option<&crate::parsing::examples::ExampleTable>,
) -> Result<(), TokenStream> {
    let Some(ex) = examples else {
        return Ok(());
    };

    let mut seen = std::collections::HashSet::new();
    for header in &ex.headers {
        if !seen.insert(header.clone()) {
            let err = syn::Error::new(
                proc_macro2::Span::call_site(),
                format!("Duplicate header '{header}' found in examples table"),
            );
            return Err(error_to_tokens(&err));
        }
    }

    for header in &ex.headers {
        let matching_param = find_matching_parameter(sig, header)?;
        add_case_attribute_if_missing(matching_param);
    }
    Ok(())
}
