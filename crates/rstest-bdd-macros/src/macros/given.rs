//! Implementation of the `#[given]` macro.

use proc_macro::TokenStream;

/// Macro for defining a Given step that registers with the step inventory.
pub(crate) fn given(attr: TokenStream, item: TokenStream) -> TokenStream {
    super::step_attr(attr, item, crate::StepKeyword::Given)
}
