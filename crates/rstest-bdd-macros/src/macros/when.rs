//! Implementation of the `#[when]` macro.

use proc_macro::TokenStream;

/// Macro for defining a When step that registers with the step inventory.
pub(crate) fn when(attr: TokenStream, item: TokenStream) -> TokenStream {
    super::step_attr(attr, item, crate::StepKeyword::When)
}
