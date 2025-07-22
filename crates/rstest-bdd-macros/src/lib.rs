//! Procedural macros for rstest-bdd.
//!
//! This crate provides attribute macros for annotating BDD test steps and
//! scenarios. The macros currently act as markers only, allowing compile-time
//! validation that annotated functions use the expected signatures. Future
//! versions will expand these annotations into executable test harness code.

use proc_macro::TokenStream;

/// No-op macro for defining a Given step.
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
pub fn given(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// No-op macro for defining a When step.
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
pub fn when(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// No-op macro for defining a Then step.
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
pub fn then(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// No-op macro for binding a scenario to a feature file.
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
