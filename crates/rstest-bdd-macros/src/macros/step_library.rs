//! Expansion for named, lexical step libraries.

use proc_macro::TokenStream;
use quote::{format_ident, quote};

/// Declare a module as a step library and publish its selection marker.
pub(crate) fn step_library(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let module = syn::parse_macro_input!(item as syn::ItemMod);
    let ident = &module.ident;
    let visibility = &module.vis;
    let marker = format_ident!("__RSTEST_BDD_STEP_LIBRARY_{ident}");
    let path = crate::codegen::rstest_bdd_path();
    let module_tokens = module.content.as_ref().map_or_else(
        || quote! { #module },
        |(_, items)| {
            let outer_attributes = module
                .attrs
                .iter()
                .filter(|attribute| matches!(attribute.style, syn::AttrStyle::Outer));
            quote! {
                #(#outer_attributes)*
                #visibility mod #ident {
                    //! Named step-library vocabulary.

                    #(#items)*
                }
            }
        },
    );

    quote! {
        #module_tokens

        #[doc(hidden)]
        #visibility const #marker: #path::StepLibraryId =
            #path::StepLibraryId::new(concat!(module_path!(), "::", stringify!(#ident)));

        #path::submit! {
            #path::StepLibrary {
                id: #marker,
                name: stringify!(#ident),
            }
        }
    }
    .into()
}
