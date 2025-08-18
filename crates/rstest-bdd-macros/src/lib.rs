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

/// Discover all `.feature` files under the given directory and generate one
/// test per Gherkin `Scenario`.
///
/// Path semantics:
/// - The `dir` argument must be a string literal.
/// - It is resolved relative to `CARGO_MANIFEST_DIR` at macro-expansion time.
///
/// Expansion:
/// - Emits a module named after `dir` (sanitised) containing one test function
///   per discovered scenario.
/// - Each generated test executes the matched steps via the registered
///   `#[given]`, `#[when]`, and `#[then]` functions.
///
/// Example:
/// ```
/// use rstest_bdd_macros::{given, when, then, scenarios};
///
/// # #[given("a precondition")] fn precondition() {}
/// # #[when("an action occurs")] fn action() {}
/// # #[then("events are recorded")] fn events_recorded() {}
/// scenarios!("tests/features/auto");
/// ```
///
/// Errors:
/// - Emits a compile error if the directory does not exist, contains no
///   `.feature` files, or if parsing fails.
#[proc_macro]
pub fn scenarios(input: TokenStream) -> TokenStream {
    macros::scenarios(input)
}
