//! Expansion for named, lexical step libraries.

use proc_macro::TokenStream;
use quote::{format_ident, quote};

/// Attribute injected into nested items so their step macros retain lexical scope.
const INTERNAL_LIBRARY_ATTRIBUTE: &str = "rstest_bdd_internal_step_library";

/// Declare a module as a step library and publish its selection marker.
pub(crate) fn step_library(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let module = syn::parse_macro_input!(item as syn::ItemMod);
    expand_step_library(module).into()
}

/// Expand a parsed step-library module.
fn expand_step_library(mut module: syn::ItemMod) -> proc_macro2::TokenStream {
    let library_path = take_library_path(&mut module).unwrap_or_else(|| module.ident.to_string());
    if let Some((_, items)) = &mut module.content {
        annotate_step_items(items, &library_path);
    }
    let ident = &module.ident;
    let visibility = &module.vis;
    let marker = format_ident!("__RSTEST_BDD_STEP_LIBRARY_{ident}");
    let path = crate::codegen::rstest_bdd_path();
    let module_tokens = module.content.as_ref().map_or_else(
        || quote! { #module },
        |(_, items)| {
            let (outer_attributes, inner_attributes): (Vec<_>, Vec<_>) = module
                .attrs
                .iter()
                .partition(|attribute| matches!(attribute.style, syn::AttrStyle::Outer));
            quote! {
                #(#outer_attributes)*
                #visibility mod #ident {
                    #(#inner_attributes)*
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
}

/// Extract an inherited nearest-library path from a module attribute.
fn take_library_path(module: &mut syn::ItemMod) -> Option<String> {
    let index = module
        .attrs
        .iter()
        .position(|attribute| attribute.path().is_ident(INTERNAL_LIBRARY_ATTRIBUTE))?;
    let attribute = module.attrs.remove(index);
    match attribute.meta {
        syn::Meta::NameValue(value) => {
            let syn::Expr::Lit(expression) = value.value else {
                return None;
            };
            let syn::Lit::Str(path) = expression.lit else {
                return None;
            };
            Some(path.value())
        }
        _ => None,
    }
}

/// Attach the nearest lexical library to inline step definitions.
fn annotate_step_items(items: &mut [syn::Item], library_path: &str) {
    for item in items {
        match item {
            syn::Item::Fn(function) if is_step_definition(&function.attrs) => {
                annotate_item(&mut function.attrs, library_path);
            }
            syn::Item::Mod(module) if is_step_library(&module.attrs) => {
                let nested_path = format!("{library_path}::{}", module.ident);
                annotate_item(&mut module.attrs, &nested_path);
            }
            syn::Item::Mod(module) => {
                if let Some((_, nested)) = &mut module.content {
                    annotate_step_items(nested, library_path);
                }
            }
            _ => {}
        }
    }
}

/// Determine whether an item carries a supported step-definition attribute.
fn is_step_definition(attributes: &[syn::Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        ["given", "when", "then"]
            .iter()
            .any(|name| attribute.path().is_ident(name))
    })
}

/// Determine whether a nested module declares a new nearest library boundary.
fn is_step_library(attributes: &[syn::Attribute]) -> bool {
    attributes
        .iter()
        .any(|attribute| attribute.path().is_ident("step_library"))
}

/// Attach an inert attribute consumed by the step attribute macro.
fn annotate_item(attributes: &mut Vec<syn::Attribute>, library_path: &str) {
    if attributes
        .iter()
        .any(|attribute| attribute.path().is_ident(INTERNAL_LIBRARY_ATTRIBUTE))
    {
        return;
    }
    let path = syn::LitStr::new(library_path, proc_macro2::Span::call_site());
    attributes.push(syn::parse_quote!(#[rstest_bdd_internal_step_library = #path]));
}

#[cfg(test)]
mod tests {
    //! Regression tests for step-library module reconstruction.

    use super::*;

    #[test]
    fn inline_expansion_preserves_outer_and_inner_attributes() {
        let item = quote! {
            #[cfg(feature = "outer")]
            mod accounts {
                #![cfg(any(unix, windows))]
                #![allow(dead_code)]
                //! User-written library documentation.

                fn helper() {}
            }
        };

        let module = syn::parse2::<syn::ItemMod>(item).expect("input step library");
        let expanded = expand_step_library(module);
        let file = syn::parse2::<syn::File>(expanded).expect("expanded step library");
        let module = file
            .items
            .iter()
            .find_map(|item| match item {
                syn::Item::Mod(module) if module.ident == "accounts" => Some(module),
                _ => None,
            })
            .expect("reconstructed accounts module");

        assert!(module.attrs.iter().any(|attribute| {
            matches!(attribute.style, syn::AttrStyle::Outer) && attribute.path().is_ident("cfg")
        }));
        assert_eq!(
            module
                .attrs
                .iter()
                .filter(|attribute| matches!(attribute.style, syn::AttrStyle::Inner(_)))
                .count(),
            4,
            "cfg, allow, user documentation, and generated documentation must remain inner"
        );
    }
}
