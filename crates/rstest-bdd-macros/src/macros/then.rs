//! Implementation of the `#[then]` macro.

use proc_macro::TokenStream;

/// Macro for defining a Then step that registers with the step inventory.
pub(crate) fn then(attr: TokenStream, item: TokenStream) -> TokenStream {
    super::step_attr(attr, item, crate::StepKeyword::Then)
}
