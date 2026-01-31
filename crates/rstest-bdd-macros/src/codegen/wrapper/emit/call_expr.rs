//! Call expression generation based on step return kind.

use crate::return_classifier::ReturnKind;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

/// Generate the call expression for a step function based on its return kind.
///
/// This helper emits the token stream that invokes the user's step function
/// and wraps the result according to the inferred [`ReturnKind`]:
///
/// - [`ReturnKind::Unit`]: Discards the return value and yields `Ok(None)`.
/// - [`ReturnKind::Value`]: Wraps the value via `__rstest_bdd_payload_from_value`,
///   which boxes non-unit values or returns `None` for unit.
/// - [`ReturnKind::ResultUnit`] / [`ReturnKind::ResultValue`]: Unpacks the
///   `Result`, mapping `Ok(value)` through the payload helper and converting
///   `Err(e)` to a `String` for the step error.
pub(super) fn generate_call_expression(
    return_kind: ReturnKind,
    ident: &syn::Ident,
    arg_idents: &[syn::Ident],
    is_async: bool,
) -> TokenStream2 {
    let path = crate::codegen::rstest_bdd_path();
    let call = if is_async {
        quote! { #ident(#(#arg_idents),*).await }
    } else {
        quote! { #ident(#(#arg_idents),*) }
    };
    match return_kind {
        ReturnKind::Unit => quote! {{
            #call;
            Ok(None)
        }},
        ReturnKind::Value => quote! {
            Ok(#path::__rstest_bdd_payload_from_value(#call))
        },
        ReturnKind::ResultUnit | ReturnKind::ResultValue => quote! {{
            match #call {
                ::core::result::Result::Ok(value) => Ok(#path::__rstest_bdd_payload_from_value(value)),
                ::core::result::Result::Err(error) => Err(error.to_string()),
            }
        }},
    }
}
