//! Procedural macros for rstest-bdd.
//!
//! This crate provides attribute macros for annotating BDD test steps and
//! scenarios. The step macros register annotated functions with the global
//! step inventory system, enabling runtime discovery and execution of step
//! definitions.

use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, LitStr, parse_macro_input};

fn step_attr(attr: TokenStream, item: TokenStream, keyword: &str) -> TokenStream {
    let pattern = parse_macro_input!(attr as LitStr);
    let func = parse_macro_input!(item as ItemFn);
    let ident = &func.sig.ident;

    TokenStream::from(quote! {
        #func
        rstest_bdd::step!(#keyword, #pattern, #ident);
    })
}

/// Macro for defining a Given step that registers with the step inventory.
///
/// *attr* The string literal specifies the text of the `Given` step as it
/// appears in the feature file.
///
/// # Examples
///
/// ```
/// use rstest_bdd_macros::given;
///
/// #[given("a user is logged in")]
/// fn user_logged_in() {
///     // setup code
/// }
/// ```
#[proc_macro_attribute]
pub fn given(attr: TokenStream, item: TokenStream) -> TokenStream {
    step_attr(attr, item, "Given")
}

/// Macro for defining a When step that registers with the step inventory.
///
/// *attr* The string literal specifies the text of the `When` step as it
/// appears in the feature file.
///
/// # Examples
///
/// ```
/// use rstest_bdd_macros::when;
///
/// #[when("the user clicks login")]
/// fn user_clicks_login() {
///     // action code
/// }
/// ```
#[proc_macro_attribute]
pub fn when(attr: TokenStream, item: TokenStream) -> TokenStream {
    step_attr(attr, item, "When")
}

/// Macro for defining a Then step that registers with the step inventory.
///
/// *attr* The string literal specifies the text of the `Then` step as it
/// appears in the feature file.
///
/// # Examples
///
/// ```
/// use rstest_bdd_macros::then;
///
/// #[then("the user should be redirected")]
/// fn user_redirected() {
///     // assertion code
/// }
/// ```
#[proc_macro_attribute]
pub fn then(attr: TokenStream, item: TokenStream) -> TokenStream {
    step_attr(attr, item, "Then")
}

/// No-op macro for binding a scenario to a feature file.
///
/// *attr* The string literal gives the path to the feature file containing the
/// scenario.
///
/// # Examples
///
/// ```
/// use rstest_bdd_macros::scenario;
///
/// #[scenario("user_login.feature")]
/// fn test_user_login() {
///     // test implementation
/// }
/// ```
#[proc_macro_attribute]
pub fn scenario(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}
