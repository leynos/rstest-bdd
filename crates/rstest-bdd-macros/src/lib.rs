//! Procedural macros for rstest-bdd.
//!
//! This crate provides attribute macros for annotating BDD test steps and
//! scenarios. The step macros register annotated functions with the global
//! step inventory system, enabling runtime discovery and execution of step
//! definitions.

use gherkin::{Feature, GherkinEnv, StepType};
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use syn::parse::{Parse, ParseStream};
use syn::token::{Comma, Eq};
use syn::{ItemFn, LitInt, LitStr, Result, parse_macro_input};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct ScenarioArgs {
    path: LitStr,
    index: Option<usize>,
}

impl Parse for ScenarioArgs {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        if input.peek(LitStr) {
            Self::parse_bare_string(input)
        } else {
            Self::parse_named_args(input)
        }
    }
}

impl ScenarioArgs {
    fn parse_bare_string(input: ParseStream<'_>) -> Result<Self> {
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

        Ok(Self { path, index })
    }

    fn parse_named_args(input: ParseStream<'_>) -> Result<Self> {
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
    let mut func = parse_macro_input!(item as ItemFn);
    let ident = &func.sig.ident;

    let mut args = Vec::new();

    for input in &mut func.sig.inputs {
        let syn::FnArg::Typed(arg) = input else {
            return syn::Error::new_spanned(input, "methods not supported")
                .to_compile_error()
                .into();
        };

        let mut fixture_name = None;
        arg.attrs.retain(|a| {
            if a.path().is_ident("from") {
                fixture_name = a.parse_args::<syn::Ident>().ok();
                false
            } else {
                true
            }
        });

        let pat = match &*arg.pat {
            syn::Pat::Ident(i) => i.ident.clone(),
            _ => {
                return syn::Error::new_spanned(&arg.pat, "unsupported pattern")
                    .to_compile_error()
                    .into();
            }
        };

        let name = fixture_name.unwrap_or_else(|| pat.clone());
        args.push((pat, name, (*arg.ty).clone()));
    }

    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let wrapper_ident = format_ident!("__rstest_bdd_wrapper_{}_{}", ident, id);
    let const_ident = format_ident!("__rstest_bdd_fixtures_{}_{}", ident, id);

    let declares = args.iter().map(|(pat, name, ty)| {
        if matches!(ty, syn::Type::Reference(_)) {
            quote! {
                let #pat: #ty = ctx
                    .get::<#ty>(stringify!(#name))
                    .expect("missing fixture");
            }
        } else {
            quote! {
                let #pat: #ty = ctx
                    .get::<#ty>(stringify!(#name))
                    .expect("missing fixture")
                    .clone();
            }
        }
    });
    let arg_idents = args.iter().map(|(pat, _, _)| pat);
    let fixture_names: Vec<_> = args
        .iter()
        .map(|(_, name, _)| {
            let s = name.to_string();
            quote! { #s }
        })
        .collect();
    let fixture_len = fixture_names.len();

    TokenStream::from(quote! {
        #func

        fn #wrapper_ident(ctx: &rstest_bdd::StepContext<'_>) {
            #(#declares)*
            #ident(#(#arg_idents),*);
        }

        #[allow(non_upper_case_globals)]
        const #const_ident: [&'static str; #fixture_len] = [#(#fixture_names),*];
        const _: [(); #fixture_len] = [(); #const_ident.len()];

        rstest_bdd::step!(#keyword, #pattern, #wrapper_ident, &#const_ident);
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
///
/// The generated test runs all scenario steps before executing the original
/// function body. Use the function block for additional assertions after the
/// steps complete.
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
        return TokenStream::from(quote! {
            compile_error!(
                "CARGO_MANIFEST_DIR is not set. This variable is normally provided by Cargo. \
                 Ensure the macro runs within a Cargo build context."
            );
        });
    };
    let feature_path = PathBuf::from(manifest_dir).join(&path);

    let feature_path_str = feature_path.display().to_string();
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

    let scenario_name = scenario.name.clone();
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

    let arg_idents: Vec<syn::Ident> = sig
        .inputs
        .iter()
        .filter_map(|arg| match arg {
            syn::FnArg::Typed(p) => match &*p.pat {
                syn::Pat::Ident(id) => Some(id.ident.clone()),
                _ => None,
            },
            syn::FnArg::Receiver(_) => None,
        })
        .collect();

    let ctx_inserts = arg_idents
        .iter()
        .map(|id| quote! { ctx.insert(stringify!(#id), &#id); });

    TokenStream::from(quote! {
        #(#attrs)*
        #[rstest::rstest]
        #vis #sig {
            let steps = [#((#keywords, #values)),*];
            let mut ctx = rstest_bdd::StepContext::default();
            #(#ctx_inserts)*
            for (index, (keyword, text)) in steps.iter().enumerate() {
                if let Some(f) = rstest_bdd::lookup_step(keyword, text) {
                    f(&ctx);
                } else {
                    panic!(
                        "Step not found at index {}: {} {} (feature: {}, scenario: {})",
                        index,
                        keyword,
                        text,
                        #feature_path_str,
                        #scenario_name
                    );
                }
            }
            // Execute the original function body after all scenario steps complete
            #block
        }
    })
}
