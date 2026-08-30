//! Call expression generation based on step return kind.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

use crate::return_classifier::StepReturnStrategy;

/// Generate the call expression for a step function based on its return strategy.
///
/// This helper emits the token stream that invokes the user's step function,
/// then either applies an explicit hint or asks the compiler to classify its
/// concrete return type.
///
/// When `is_async` is `true`, the generated call expression includes `.await`.
pub(super) fn generate_call_expression(
    strategy: StepReturnStrategy,
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
    match strategy {
        StepReturnStrategy::Unit => quote! {{
            #call;
            Ok(None)
        }},
        StepReturnStrategy::Never => quote! {{
            #call;
            unreachable!()
        }},
        StepReturnStrategy::ForcedValue => quote! {
            Ok(#path::__rstest_bdd_payload_from_value(#call))
        },
        StepReturnStrategy::ForcedResult => quote! {{
            match #call {
                ::core::result::Result::Ok(value) => Ok(#path::__rstest_bdd_payload_from_value(value)),
                ::core::result::Result::Err(error) => Err(error.to_string()),
            }
        }},
        StepReturnStrategy::Dispatch => quote! {{
            use #path::step_return::StepReturnValueKind as _;
            let __rstest_bdd_step_return_value = #call;
            let __rstest_bdd_step_return_tag = #path::step_return::StepReturnProbe(
                &__rstest_bdd_step_return_value,
            )
            .__rstest_bdd_step_return_kind();
            #path::step_return::StepReturnNormalize::normalize(
                __rstest_bdd_step_return_tag,
                __rstest_bdd_step_return_value,
            )
        }},
    }
}

#[cfg(test)]
mod tests {
    //! Snapshot coverage for the type-directed dispatch emission.

    use insta::assert_snapshot;
    use quote::format_ident;

    use super::{StepReturnStrategy, generate_call_expression};

    /// Verifies the emitted dispatch expression through its approved snapshot.
    #[test]
    fn dispatches_through_the_runtime_probe() {
        let ident = format_ident!("step");
        let tokens = generate_call_expression(StepReturnStrategy::Dispatch, &ident, &[], false);

        assert_snapshot!(tokens.to_string());
    }
}
