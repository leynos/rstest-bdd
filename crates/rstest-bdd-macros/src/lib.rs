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
use syn::parse::{Parse, ParseStream};
use syn::token::{Comma, Eq};
use syn::{ItemFn, LitInt, LitStr, Result, parse_macro_input};

struct ScenarioArgs {
    path: LitStr,
    index: Option<usize>,
}

impl Parse for ScenarioArgs {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        if input.peek(LitStr) {
            let path: LitStr = input.parse()?;
            let mut index = None;

            if input.peek(Comma) {
                input.parse::<Comma>()?;
                let ident: syn::Ident = input.parse()?;
                if ident != "index" {
                    return Err(input.error("expected `index`"));
                }
                input.parse::<Eq>()?;
                let lit: LitInt = input.parse()?;
                index = Some(lit.base10_parse()?);
            }

            if !input.is_empty() {
                return Err(input.error("unexpected tokens"));
            }

            return Ok(Self { path, index });
        }

        let mut path = None;
        let mut index = None;

        while !input.is_empty() {
            let ident: syn::Ident = input.parse()?;
            input.parse::<Eq>()?;
            if ident == "path" {
                let lit: LitStr = input.parse()?;
                path = Some(lit);
            } else if ident == "index" {
                let lit: LitInt = input.parse()?;
                index = Some(lit.base10_parse()?);
            } else {
                return Err(input.error("expected `path` or `index`"));
            }

            if input.peek(Comma) {
                input.parse::<Comma>()?;
            } else {
                break;
            }
        }

        let Some(path) = path else {
            return Err(input.error("`path` is required"));
        };

        if !input.is_empty() {
            return Err(input.error("unexpected tokens"));
        }

        Ok(Self { path, index })
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

/// Bind a test to a scenario defined in a feature file.
///
/// *attr* Accepts either a bare string literal giving the path to the feature
/// file or a `path = "..."` argument. An optional `index = N` argument selects
/// which scenario to run when the file contains more than one.
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
///
/// #[scenario(path = "user_login.feature", index = 1)]
/// fn second_case() {}
/// ```
///
/// # Panics
///
/// This macro does not panic. Invalid input results in a compile error.
#[proc_macro_attribute]
pub fn scenario(attr: TokenStream, item: TokenStream) -> TokenStream {
    let ScenarioArgs { path, index } = parse_macro_input!(attr as ScenarioArgs);
    let path = PathBuf::from(path.value());

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
    let Some(scenario) = feature.scenarios.get(index.unwrap_or(0)) else {
        return TokenStream::from(quote! { compile_error!("scenario index out of range") });
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
                if let Some(f) = rstest_bdd::lookup_step(keyword, text) {
                    f();
                } else {
                    panic!("Step not found: {} {}", keyword, text);
                }
            }
            #block
        }
    })
}
