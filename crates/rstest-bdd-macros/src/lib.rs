//! Procedural macros for rstest-bdd.

mod args;
mod codegen;
mod feature;
mod scenario;
mod step;

use proc_macro::TokenStream;

/// Register a `Given` step.
///
/// ```
/// use rstest_bdd_macros::given;
/// #[given("a precondition")]
/// fn setup() {}
/// ```
#[proc_macro_attribute]
pub fn given(attr: TokenStream, item: TokenStream) -> TokenStream {
    step::step_attr(attr, item, rstest_bdd::StepKeyword::Given)
}

/// Register a `When` step.
///
/// ```
/// use rstest_bdd_macros::when;
/// #[when("an action occurs")]
/// fn act() {}
/// ```
#[proc_macro_attribute]
pub fn when(attr: TokenStream, item: TokenStream) -> TokenStream {
    step::step_attr(attr, item, rstest_bdd::StepKeyword::When)
}

/// Register a `Then` step.
///
/// ```
/// use rstest_bdd_macros::then;
/// #[then("expect something")]
/// fn check() {}
/// ```
#[proc_macro_attribute]
pub fn then(attr: TokenStream, item: TokenStream) -> TokenStream {
    step::step_attr(attr, item, rstest_bdd::StepKeyword::Then)
}

/// Bind a test to a scenario defined in a feature file.
///
/// ```ignore
/// use rstest_bdd_macros::scenario;
/// #[scenario("path/to.feature")]
/// fn run() {}
/// ```
#[proc_macro_attribute]
pub fn scenario(attr: TokenStream, item: TokenStream) -> TokenStream {
    scenario::scenario(attr, item)
}
