//! Procedural macros for rstest-bdd.

use proc_macro::TokenStream;

/// No-op macro for defining a Given step.
#[proc_macro_attribute]
pub fn given(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// No-op macro for defining a When step.
#[proc_macro_attribute]
pub fn when(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// No-op macro for defining a Then step.
#[proc_macro_attribute]
pub fn then(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// No-op macro for binding a scenario to a feature file.
#[proc_macro_attribute]
pub fn scenario(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}
