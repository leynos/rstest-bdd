//! Attribute macros enabling Behaviour-Driven testing with `rstest`.
//!
//! The macros in this crate parse Gherkin feature files and generate
//! parameterized test functions. Step definitions are registered via an
//! inventory to allow the runner to discover them at runtime.

use gherkin::{Feature, GherkinEnv, StepType};
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use syn::parse::{Parse, ParseStream};
use syn::token::{Comma, Eq};
use syn::{ItemFn, LitInt, LitStr, parse_macro_input};

/// Convert a `syn::Error` into a `TokenStream` for macro errors.
fn error_to_tokens(err: &syn::Error) -> TokenStream {
    err.to_compile_error().into()
}

/// Create a `LitStr` from an examples table cell, escaping as needed.
fn cell_to_lit(value: &str) -> syn::LitStr {
    syn::LitStr::new(value, proc_macro2::Span::call_site())
}

struct FixtureArg {
    pat: syn::Ident,
    name: syn::Ident,
    ty: syn::Type,
}

fn extract_fixture_args(func: &mut ItemFn) -> syn::Result<Vec<FixtureArg>> {
    let mut args = Vec::new();

    for input in &mut func.sig.inputs {
        let syn::FnArg::Typed(arg) = input else {
            return Err(syn::Error::new_spanned(input, "methods not supported"));
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
                return Err(syn::Error::new_spanned(&arg.pat, "unsupported pattern"));
            }
        };

        let name = fixture_name.unwrap_or_else(|| pat.clone());
        args.push(FixtureArg {
            pat,
            name,
            ty: (*arg.ty).clone(),
        });
    }

    Ok(args)
}

fn generate_wrapper_code(
    ident: &syn::Ident,
    args: &[FixtureArg],
    pattern: &LitStr,
    keyword: &str,
) -> TokenStream2 {
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let wrapper_ident = format_ident!("__rstest_bdd_wrapper_{}_{}", ident, id);
    let const_ident = format_ident!("__rstest_bdd_fixtures_{}_{}", ident, id);

    let declares = args.iter().map(|FixtureArg { pat, name, ty }| {
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

    let arg_idents = args.iter().map(|FixtureArg { pat, .. }| pat);
    let fixture_names: Vec<_> = args
        .iter()
        .map(|FixtureArg { name, .. }| {
            let s = name.to_string();
            quote! { #s }
        })
        .collect();
    let fixture_len = fixture_names.len();

    quote! {
        fn #wrapper_ident(ctx: &rstest_bdd::StepContext<'_>) {
            #(#declares)*
            #ident(#(#arg_idents),*);
        }

        #[allow(non_upper_case_globals)]
        const #const_ident: [&'static str; #fixture_len] = [#(#fixture_names),*];
        const _: [(); #fixture_len] = [(); #const_ident.len()];

        rstest_bdd::step!(#keyword, #pattern, #wrapper_ident, &#const_ident);
    }
}

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct ScenarioArgs {
    path: LitStr,
    index: Option<usize>,
}

impl Parse for ScenarioArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        if input.peek(LitStr) {
            Self::parse_bare_string(input)
        } else {
            Self::parse_named_args(input)
        }
    }
}

impl ScenarioArgs {
    fn parse_bare_string(input: ParseStream<'_>) -> syn::Result<Self> {
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

    fn parse_named_args(input: ParseStream<'_>) -> syn::Result<Self> {
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

    let args = match extract_fixture_args(&mut func) {
        Ok(args) => args,
        Err(err) => return error_to_tokens(&err),
    };

    let ident = &func.sig.ident;

    let wrapper_code = generate_wrapper_code(ident, &args, &pattern, keyword);

    TokenStream::from(quote! {
        #func
        #wrapper_code
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

/// Validate Examples table structure in feature file text
fn validate_examples_in_feature_text(text: &str) -> Result<(), TokenStream> {
    if !text.contains("Examples:") {
        return Ok(());
    }

    let examples_idx = find_examples_table_start(text)?;
    validate_table_column_consistency(text, examples_idx)
}

/// Find the starting line index of the Examples table
fn find_examples_table_start(text: &str) -> Result<usize, TokenStream> {
    text.lines()
        .enumerate()
        .find(|(_, line)| line.trim_start().starts_with("Examples:"))
        .map(|(idx, _)| idx)
        .ok_or_else(|| {
            error_to_tokens(&syn::Error::new(
                proc_macro2::Span::call_site(),
                "Examples table structure error",
            ))
        })
}

/// Validate that all example rows have consistent column counts with header
fn validate_table_column_consistency(text: &str, start_idx: usize) -> Result<(), TokenStream> {
    let mut table_rows = text
        .lines()
        .skip(start_idx + 1)
        .take_while(|line| line.trim_start().starts_with('|'));

    let Some(header_row) = table_rows.next() else {
        return Ok(());
    };

    let expected_columns = count_non_empty_columns(header_row);

    for data_row in table_rows {
        let actual_columns = count_non_empty_columns(data_row);
        if actual_columns < expected_columns {
            return Err(error_to_tokens(&syn::Error::new(
                proc_macro2::Span::call_site(),
                "Example row has fewer columns than header row in Examples table",
            )));
        }
    }

    Ok(())
}

/// Count non-empty columns in a table row by splitting on '|'
fn count_non_empty_columns(row: &str) -> usize {
    row.split('|')
        .filter(|cell| !cell.trim().is_empty())
        .count()
}

fn parse_and_load_feature(path: &Path) -> Result<Feature, TokenStream> {
    let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") else {
        let err = syn::Error::new(
            proc_macro2::Span::call_site(),
            "CARGO_MANIFEST_DIR is not set. This variable is normally provided by Cargo. Ensure the macro runs within a Cargo build context.",
        );
        return Err(error_to_tokens(&err));
    };
    let feature_path = PathBuf::from(manifest_dir).join(path);
    Feature::parse_path(&feature_path, GherkinEnv::default()).map_err(|err| {
        if let Ok(text) = std::fs::read_to_string(&feature_path) {
            if let Err(validation_err) = validate_examples_in_feature_text(&text) {
                return validation_err;
            }
        }
        let msg = format!("failed to parse feature file: {err}");
        error_to_tokens(&syn::Error::new(proc_macro2::Span::call_site(), msg))
    })
}

/// Rows parsed from a `Scenario Outline` examples table.
struct ExampleTable {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
}

/// Name, steps, and optional examples extracted from a Gherkin scenario.
struct ScenarioData {
    name: String,
    steps: Vec<(String, String)>,
    examples: Option<ExampleTable>,
}

fn extract_examples(scenario: &gherkin::Scenario) -> Result<Option<ExampleTable>, TokenStream> {
    if !should_process_outline(scenario) {
        return Ok(None);
    }

    let first_table = get_first_examples_table(scenario)?;
    let headers = extract_and_validate_headers(first_table)?;
    validate_header_consistency(scenario, &headers)?;
    let rows = flatten_and_validate_rows(scenario, headers.len())?;

    Ok(Some(ExampleTable { headers, rows }))
}

fn should_process_outline(scenario: &gherkin::Scenario) -> bool {
    scenario.keyword == "Scenario Outline" || !scenario.examples.is_empty()
}

fn get_first_examples_table(scenario: &gherkin::Scenario) -> Result<&gherkin::Table, TokenStream> {
    scenario
        .examples
        .first()
        .and_then(|ex| ex.table.as_ref())
        .ok_or_else(|| {
            error_to_tokens(&syn::Error::new(
                proc_macro2::Span::call_site(),
                "Scenario Outline missing Examples table",
            ))
        })
}

fn extract_and_validate_headers(table: &gherkin::Table) -> Result<Vec<String>, TokenStream> {
    let first = table.rows.first().ok_or_else(|| {
        error_to_tokens(&syn::Error::new(
            proc_macro2::Span::call_site(),
            "Examples table must have at least one row",
        ))
    })?;
    Ok(first.clone())
}

fn validate_header_consistency(
    scenario: &gherkin::Scenario,
    expected_headers: &[String],
) -> Result<(), TokenStream> {
    for ex in scenario.examples.iter().skip(1) {
        let table = ex.table.as_ref().ok_or_else(|| {
            error_to_tokens(&syn::Error::new(
                proc_macro2::Span::call_site(),
                "Examples table missing rows",
            ))
        })?;
        let headers = table.rows.first().ok_or_else(|| {
            error_to_tokens(&syn::Error::new(
                proc_macro2::Span::call_site(),
                "Examples table must have at least one row",
            ))
        })?;
        if headers != expected_headers {
            return Err(error_to_tokens(&syn::Error::new(
                proc_macro2::Span::call_site(),
                "All Examples tables must have the same headers",
            )));
        }
    }
    Ok(())
}

fn flatten_and_validate_rows(
    scenario: &gherkin::Scenario,
    expected_width: usize,
) -> Result<Vec<Vec<String>>, TokenStream> {
    let rows: Vec<Vec<String>> = scenario
        .examples
        .iter()
        .filter_map(|ex| ex.table.as_ref())
        .flat_map(|t| t.rows.iter().skip(1).cloned())
        .collect();

    for (i, row) in rows.iter().enumerate() {
        if row.len() != expected_width {
            let err = syn::Error::new(
                proc_macro2::Span::call_site(),
                format!(
                    "Malformed examples table: row {} has {} columns, expected {}",
                    i + 2,
                    row.len(),
                    expected_width
                ),
            );
            return Err(error_to_tokens(&err));
        }
    }

    Ok(rows)
}

fn extract_scenario_steps(
    feature: &Feature,
    index: Option<usize>,
) -> Result<ScenarioData, TokenStream> {
    let Some(scenario) = feature.scenarios.get(index.unwrap_or(0)) else {
        let err = syn::Error::new(
            proc_macro2::Span::call_site(),
            "scenario index out of range",
        );
        return Err(error_to_tokens(&err));
    };

    let scenario_name = scenario.name.clone();
    let steps = scenario
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

    let examples = extract_examples(scenario)?;

    Ok(ScenarioData {
        name: scenario_name,
        steps,
        examples,
    })
}

fn extract_function_fixtures(
    sig: &syn::Signature,
) -> (Vec<syn::Ident>, impl Iterator<Item = TokenStream2>) {
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

    let inserts: Vec<_> = arg_idents
        .iter()
        .map(|id| quote! { ctx.insert(stringify!(#id), &#id); })
        .collect();

    (arg_idents, inserts.into_iter())
}

fn generate_case_attrs(examples: &ExampleTable) -> Vec<TokenStream2> {
    examples
        .rows
        .iter()
        .map(|row| {
            let cells = row.iter().map(|v| {
                let lit = cell_to_lit(v);
                quote! { #lit }
            });
            quote! { #[case( #(#cells),* )] }
        })
        .collect()
}

#[expect(
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    reason = "signature defined by requirements"
)]
fn generate_scenario_code(
    attrs: &[syn::Attribute],
    vis: &syn::Visibility,
    sig: &syn::Signature,
    block: &syn::Block,
    feature_path_str: String,
    scenario_name: String,
    steps: Vec<(String, String)>,
    examples: Option<ExampleTable>,
    ctx_inserts: impl Iterator<Item = TokenStream2>,
) -> TokenStream {
    let keywords = steps.iter().map(|(k, _)| k);
    let values = steps.iter().map(|(_, v)| v);

    let case_attrs = examples.map_or_else(Vec::new, |ex| generate_case_attrs(&ex));

    TokenStream::from(quote! {
        #[rstest::rstest]
        #(#case_attrs)*
        #(#attrs)*
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
            #block
        }
    })
}

/// Check if a function parameter matches the given header name.
fn parameter_matches_header(arg: &syn::FnArg, header: &str) -> bool {
    match arg {
        syn::FnArg::Typed(p) => match &*p.pat {
            syn::Pat::Ident(id) => id.ident == *header,
            _ => false,
        },
        syn::FnArg::Receiver(_) => false,
    }
}

/// Find a function parameter matching the given header name.
fn find_matching_parameter<'a>(
    sig: &'a mut syn::Signature,
    header: &str,
) -> Result<&'a mut syn::FnArg, TokenStream> {
    if let Some(pos) = sig
        .inputs
        .iter()
        .position(|arg| parameter_matches_header(arg, header))
    {
        sig.inputs
            .iter_mut()
            .nth(pos)
            .map_or_else(|| unreachable!("position from earlier search exists"), Ok)
    } else {
        Err(create_parameter_mismatch_error(sig, header))
    }
}

/// Add case attribute to parameter if not already present.
fn add_case_attribute_if_missing(arg: &mut syn::FnArg) {
    if let syn::FnArg::Typed(p) = arg {
        if !has_case_attribute(p) {
            p.attrs.push(syn::parse_quote!(#[case]));
        }
    }
}

/// Check if parameter already has a case attribute.
fn has_case_attribute(p: &syn::PatType) -> bool {
    p.attrs.iter().any(|attr| {
        let segs: Vec<_> = attr
            .path()
            .segments
            .iter()
            .map(|s| s.ident.to_string())
            .collect();
        segs == ["case"] || segs == ["rstest", "case"]
    })
}

/// Create error for parameter mismatch with helpful diagnostics.
fn create_parameter_mismatch_error(sig: &syn::Signature, header: &str) -> TokenStream {
    let available_params: Vec<String> = sig
        .inputs
        .iter()
        .filter_map(|arg| match arg {
            syn::FnArg::Typed(p) => match &*p.pat {
                syn::Pat::Ident(id) => Some(id.ident.to_string()),
                _ => None,
            },
            syn::FnArg::Receiver(_) => None,
        })
        .collect();
    let msg = format!(
        "parameter `{header}` not found for scenario outline column. Available parameters: [{}]",
        available_params.join(", ")
    );
    error_to_tokens(&syn::Error::new_spanned(sig, msg))
}

/// Process scenario outline examples and modify function parameters.
fn process_scenario_outline_examples(
    sig: &mut syn::Signature,
    examples: Option<&ExampleTable>,
) -> Result<(), TokenStream> {
    let Some(ex) = examples else {
        return Ok(());
    };

    {
        use std::collections::HashSet;
        let mut seen = HashSet::new();
        for header in &ex.headers {
            if !seen.insert(header.clone()) {
                let err = syn::Error::new(
                    proc_macro2::Span::call_site(),
                    format!("Duplicate header '{header}' found in examples table"),
                );
                return Err(error_to_tokens(&err));
            }
        }
    }

    for header in &ex.headers {
        let matching_param = find_matching_parameter(sig, header)?;
        add_case_attribute_if_missing(matching_param);
    }
    Ok(())
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

    let mut item_fn = parse_macro_input!(item as ItemFn);
    let attrs = &item_fn.attrs;
    let vis = &item_fn.vis;
    let sig = &mut item_fn.sig;
    let block = &item_fn.block;

    let feature = match parse_and_load_feature(&path) {
        Ok(f) => f,
        Err(err) => return err,
    };
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| String::new());
    let feature_path_str = PathBuf::from(manifest_dir)
        .join(&path)
        .display()
        .to_string();

    let ScenarioData {
        name: scenario_name,
        steps,
        examples,
    } = match extract_scenario_steps(&feature, index) {
        Ok(res) => res,
        Err(err) => return err,
    };

    if let Err(err) = process_scenario_outline_examples(sig, examples.as_ref()) {
        return err;
    }

    let (_args, ctx_inserts) = extract_function_fixtures(sig);

    generate_scenario_code(
        attrs,
        vis,
        sig,
        block,
        feature_path_str,
        scenario_name,
        steps,
        examples,
        ctx_inserts,
    )
}
