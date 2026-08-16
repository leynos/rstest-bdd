//! Test-only procedural macros for lint allowances required by macro expansion.

use proc_macro::TokenStream;
use quote::quote;
use syn::{Item, parse_macro_input};

/// Allows `unused_braces` only for an `rstest` fixture expansion.
///
/// `rstest` wraps fixture bodies in a nested block. A single-expression body
/// then trips `unused_braces` under denied warnings, while the workspace
/// formatter collapses a multi-line workaround back into that shape.
///
/// Apply this attribute immediately above `#[fixture]`. It deliberately uses
/// `allow` rather than `expect`: fixture bodies may be multi-statement, where
/// an expectation would be unfulfilled. The Clippy expectation is conditional
/// because `clippy::allow_attributes` is unavailable outside a Clippy run.
///
/// # Examples
///
/// ```
/// use rstest::fixture;
/// use rstest_bdd_test_macros::allow_fixture_expansion_lints;
///
/// #[allow_fixture_expansion_lints]
/// #[fixture]
/// fn seed() -> u32 { 7 }
/// ```
#[proc_macro_attribute]
pub fn allow_fixture_expansion_lints(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let parsed_item = parse_macro_input!(item as Item);

    quote! {
        #[allow(
            unused_braces,
            reason = "fixture macro expansion adds a redundant block around expression bodies"
        )]
        #[cfg_attr(
            clippy,
            expect(
                clippy::allow_attributes,
                reason = "the fixture expansion needs a scoped unused-braces allowance"
            )
        )]
        #parsed_item
    }
    .into()
}
