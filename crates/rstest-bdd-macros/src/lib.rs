//! Attribute macros enabling Behaviour-Driven testing with `rstest`.

mod codegen;
mod macros;
mod parsing;
mod utils;
mod validation;

use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn given(attr: TokenStream, item: TokenStream) -> TokenStream {
    macros::given(attr, item)
}

#[proc_macro_attribute]
pub fn when(attr: TokenStream, item: TokenStream) -> TokenStream {
    macros::when(attr, item)
}

#[proc_macro_attribute]
pub fn then(attr: TokenStream, item: TokenStream) -> TokenStream {
    macros::then(attr, item)
}

#[proc_macro_attribute]
pub fn scenario(attr: TokenStream, item: TokenStream) -> TokenStream {
    macros::scenario(attr, item)
}
