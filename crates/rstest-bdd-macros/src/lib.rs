//! Procedural macros for rstest-bdd.
//!
//! This crate provides attribute macros for annotating BDD test steps and
//! scenarios. The step macros register annotated functions with the global
//! step inventory system, enabling runtime discovery and execution of step
//! definitions.

use gherkin::{Feature, GherkinEnv, StepType};
use proc_macro::TokenStream;
use quote::quote;
use std::path::PathBuf;
use syn::Result;
use syn::parse::{Parse, ParseStream};
use syn::token::Eq;
use syn::{ItemFn, LitStr, parse_macro_input};

struct PathArg(LitStr);

impl Parse for PathArg {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let ident: syn::Ident = input.parse()?;
        input.parse::<Eq>()?;
        if ident != "path" {
            return Err(input.error("expected `path`"));
        }
        let lit: LitStr = input.parse()?;
        Ok(Self(lit))
    }
}

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

/// Bind a test to the first scenario in a feature file.
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
///
/// # Panics
///
/// This macro does not panic. Invalid input results in a compile error.
#[proc_macro_attribute]
pub fn scenario(attr: TokenStream, item: TokenStream) -> TokenStream {
    let PathArg(path_lit) = parse_macro_input!(attr as PathArg);
    let path = PathBuf::from(path_lit.value());

    let item_fn = parse_macro_input!(item as ItemFn);
    let attrs = &item_fn.attrs;
    let vis = &item_fn.vis;
    let sig = &item_fn.sig;
    let block = &item_fn.block;

    let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") else {
        return TokenStream::from(quote! { compile_error!("CARGO_MANIFEST_DIR not set"); });
    };
    let feature_path = PathBuf::from(manifest_dir).join(&path);

    let feature = match Feature::parse_path(&feature_path, GherkinEnv::default()) {
        Ok(f) => f,
        Err(err) => {
            let msg = format!("failed to parse feature file: {err}");
            return TokenStream::from(quote! { compile_error!(#msg); });
        }
    };
    let Some(scenario) = feature.scenarios.first() else {
        return TokenStream::from(quote! { compile_error!("feature contains no scenarios"); });
    };

    let steps: Vec<(String, String)> = scenario
        .steps
        .iter()
        .map(|s| {
            let keyword = match s.ty {
                StepType::Given => "Given",
                StepType::When => "When",
                StepType::Then => "Then",
            };
            (keyword.to_string(), s.value.clone())
        })
        .collect();

    let keywords = steps.iter().map(|(k, _)| k);
    let values = steps.iter().map(|(_, v)| v);

    TokenStream::from(quote! {
        #(#attrs)*
        #[rstest::rstest]
        #vis #sig {
            let steps = [#((#keywords, #values)),*];
            for (keyword, text) in steps {
                let mut found = false;
                for step in rstest_bdd::iter::<rstest_bdd::Step> {
                    if step.keyword == keyword && step.pattern == text {
                        (step.run)();
                        found = true;
                        break;
                    }
                }
                assert!(found, "Step not found: {} {}", keyword, text);
            }
            #block
        }
    })
}
