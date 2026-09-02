//! Parses arguments supplied to the `scenarios!` macro.
//!
//! Accepts either a positional directory literal or the `dir = "..."` and
//! `path = "..."` named arguments alongside an optional `tags = "..."` filter,
//! an optional `fixtures = [name: Type, ...]` list, and an optional
//! `runtime = "..."` mode selection.
//! The parser enforces that each input appears at most once, mirroring both
//! accepted spellings in duplicate and missing-argument diagnostics so users
//! immediately see which synonym needs adjusting.
//!
//! `RuntimeMode` and `TestAttributeHint` are imported from
//! `rstest_bdd_policy`, matching the runtime crate's public re-exports.
//! Regression tests in this module and in `rstest-bdd::execution` guard that
//! shared source of truth so macro/runtime policy semantics do not drift.

use quote::{format_ident, quote};
pub(crate) use rstest_bdd_policy::{RuntimeMode, TestAttributeHint};
use syn::{
    LitStr,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    token::Comma,
};

/// Compatibility aliases that map legacy runtime syntax to harness selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RuntimeCompatibilityAlias {
    /// Compatibility alias for the Tokio harness adapter path.
    TokioHarnessAdapter,
}

/// Provides the internal `fn` operation.
pub(super) const fn runtime_compatibility_alias(
    runtime: RuntimeMode,
) -> Option<RuntimeCompatibilityAlias> {
    match runtime {
        RuntimeMode::Sync => None,
        RuntimeMode::TokioCurrentThread => Some(RuntimeCompatibilityAlias::TokioHarnessAdapter),
    }
}

/// A single fixture specification: `name: Type`.
#[derive(Clone, Debug)]
pub(super) struct FixtureSpec {
    /// Stores the internal `name` value.
    pub(super) name: syn::Ident,
    /// Stores the internal `ty` value.
    pub(super) ty: syn::Type,
}

impl Parse for FixtureSpec {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let name: syn::Ident = input.parse()?;
        input.parse::<syn::token::Colon>()?;
        let ty: syn::Type = input.parse()?;
        Ok(Self { name, ty })
    }
}

/// Internal data used by the macros implementation.
pub(super) struct ScenariosArgs {
    /// Stores the internal `dir` value.
    pub(super) dir: LitStr,
    /// Stores the internal `tag_filter` value.
    pub(super) tag_filter: Option<LitStr>,
    /// Stores the internal `fixtures` value.
    pub(super) fixtures: Vec<FixtureSpec>,
    /// Stores the internal `runtime` value.
    pub(super) runtime: RuntimeMode,
    /// Stores the internal `harness` value.
    pub(super) harness: Option<syn::Path>,
    /// Stores the internal `attributes` value.
    pub(super) attributes: Option<syn::Path>,
    /// Closed set of step libraries selected for generated scenarios.
    pub(super) libraries: Option<Vec<syn::Path>>,
}

/// Documents the internal `ScenariosArg` item.
enum ScenariosArg {
    /// Represents the internal validation outcome.
    Dir(LitStr),
    /// Represents the internal validation outcome.
    Tags(LitStr),
    /// Represents the internal validation outcome.
    Fixtures(Vec<FixtureSpec>),
    /// Represents the internal validation outcome.
    Runtime(RuntimeMode),
    /// Represents the internal validation outcome.
    Harness(syn::Path),
    /// Represents the internal validation outcome.
    Attributes(syn::Path),
    /// Closed step-library list.
    Libraries(Vec<syn::Path>),
}

impl Parse for ScenariosArg {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        if input.peek(LitStr) {
            Ok(Self::Dir(input.parse()?))
        } else {
            let ident: syn::Ident = input.parse()?;
            input.parse::<syn::token::Eq>()?;
            parse_named_arg(&ident, input)
        }
    }
}

/// Parse a named argument based on its identifier.
fn parse_named_arg(ident: &syn::Ident, input: ParseStream<'_>) -> syn::Result<ScenariosArg> {
    match ident.to_string().as_str() {
        "dir" | "path" => Ok(ScenariosArg::Dir(input.parse()?)),
        "tags" => Ok(ScenariosArg::Tags(input.parse()?)),
        "fixtures" => parse_fixtures_arg(input),
        "runtime" => parse_runtime_arg(input),
        "harness" => Ok(ScenariosArg::Harness(input.parse()?)),
        "attributes" => Ok(ScenariosArg::Attributes(input.parse()?)),
        "libraries" => parse_libraries_arg(input),
        _ => Err(input.error(
            "expected `dir`, `path`, `tags`, `fixtures`, `runtime`, `harness`, `attributes`, or \
             `libraries`",
        )),
    }
}

/// Parse the closed `libraries = [path, ...]` selection argument.
fn parse_libraries_arg(input: ParseStream<'_>) -> syn::Result<ScenariosArg> {
    let content;
    syn::bracketed!(content in input);
    let paths = Punctuated::<syn::Path, Comma>::parse_terminated(&content)?;
    Ok(ScenariosArg::Libraries(paths.into_iter().collect()))
}

/// Parse the fixtures argument: `fixtures = [name: Type, ...]`
fn parse_fixtures_arg(input: ParseStream<'_>) -> syn::Result<ScenariosArg> {
    let content;
    syn::bracketed!(content in input);
    let specs = Punctuated::<FixtureSpec, Comma>::parse_terminated(&content)?;
    Ok(ScenariosArg::Fixtures(specs.into_iter().collect()))
}

/// Parse the runtime argument: `runtime = "tokio-current-thread"`
fn parse_runtime_arg(input: ParseStream<'_>) -> syn::Result<ScenariosArg> {
    let value: LitStr = input.parse()?;
    let mode = parse_runtime_mode(&value)?;
    Ok(ScenariosArg::Runtime(mode))
}

/// Parse a runtime mode string into a `RuntimeMode` enum.
fn parse_runtime_mode(value: &LitStr) -> syn::Result<RuntimeMode> {
    match value.value().as_str() {
        "tokio-current-thread" => Ok(RuntimeMode::TokioCurrentThread),
        other => Err(syn::Error::new(
            value.span(),
            format!("unknown runtime `{other}`; supported: \"tokio-current-thread\""),
        )),
    }
}

/// Assign `value` to `slot` if empty, or return a duplicate-argument error.
fn set_once<T>(
    slot: &mut Option<T>,
    value: T,
    label: &str,
    input: ParseStream<'_>,
) -> syn::Result<()> {
    if slot.is_some() {
        return Err(input.error(format!("duplicate `{label}` argument")));
    }
    *slot = Some(value);
    Ok(())
}

/// Process each parsed argument and populate the corresponding field.
#[expect(
    clippy::type_complexity,
    reason = "flat tuple avoids a single-use struct"
)]
fn process_args(
    args: Punctuated<ScenariosArg, Comma>,
    input: ParseStream<'_>,
) -> syn::Result<(
    Option<LitStr>,
    Option<LitStr>,
    Option<Vec<FixtureSpec>>,
    Option<RuntimeMode>,
    Option<syn::Path>,
    Option<syn::Path>,
    Option<Vec<syn::Path>>,
)> {
    let mut dir = None;
    let mut tag_filter = None;
    let mut fixtures = None;
    let mut runtime = None;
    let mut harness = None;
    let mut attributes = None;
    let mut libraries = None;

    for arg in args {
        match arg {
            ScenariosArg::Dir(lit) => set_once(&mut dir, lit, "dir/path", input)?,
            ScenariosArg::Tags(lit) => set_once(&mut tag_filter, lit, "tags", input)?,
            ScenariosArg::Fixtures(specs) => {
                set_once(&mut fixtures, specs, "fixtures", input)?;
            }
            ScenariosArg::Runtime(mode) => set_once(&mut runtime, mode, "runtime", input)?,
            ScenariosArg::Harness(p) => set_once(&mut harness, p, "harness", input)?,
            ScenariosArg::Attributes(p) => {
                set_once(&mut attributes, p, "attributes", input)?;
            }
            ScenariosArg::Libraries(paths) => {
                validate_unique_libraries(&paths)?;
                set_once(&mut libraries, paths, "libraries", input)?;
            }
        }
    }

    Ok((
        dir, tag_filter, fixtures, runtime, harness, attributes, libraries,
    ))
}

/// Reject a repeated library path before it can create duplicate candidates.
fn validate_unique_libraries(libraries: &[syn::Path]) -> syn::Result<()> {
    let mut seen = std::collections::HashSet::new();
    for library in libraries {
        let rendered = quote::quote!(#library).to_string();
        if !seen.insert(rendered) {
            return Err(syn::Error::new_spanned(
                library,
                "duplicate step library in `libraries` argument",
            ));
        }
    }
    Ok(())
}

impl Parse for ScenariosArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let args = Punctuated::<ScenariosArg, Comma>::parse_terminated(input)?;
        let (dir, tag_filter, fixtures, runtime, harness, attributes, libraries) =
            process_args(args, input)?;

        let dir = dir.ok_or_else(|| input.error("`dir` (or `path`) argument is required"))?;
        let runtime = runtime.unwrap_or_default();

        Ok(Self {
            dir,
            tag_filter,
            fixtures: fixtures.unwrap_or_default(),
            runtime,
            harness,
            attributes,
            libraries,
        })
    }
}

/// Generate the runtime scope expression for the selected module paths.
pub(super) fn library_scope_tokens(libraries: Option<&[syn::Path]>) -> proc_macro2::TokenStream {
    let runtime = crate::codegen::rstest_bdd_path();
    let Some(libraries) = libraries else {
        return quote! { #runtime::StepScope::global() };
    };
    let markers: Vec<_> = libraries.iter().map(library_marker_path).collect();
    quote! { #runtime::StepScope::new(&[#(#markers),*]) }
}

/// Convert selected Rust library paths into local validation identities.
pub(super) fn library_validation_names(libraries: &[syn::Path]) -> Vec<Box<str>> {
    libraries
        .iter()
        .map(|path| {
            path.segments
                .iter()
                .filter(|segment| {
                    !matches!(
                        segment.ident.to_string().as_str(),
                        "crate" | "self" | "super"
                    )
                })
                .map(|segment| segment.ident.to_string())
                .collect::<Vec<_>>()
                .join("::")
                .into_boxed_str()
        })
        .collect()
}

/// Convert one selected library module path into its generated marker path.
fn library_marker_path(path: &syn::Path) -> proc_macro2::TokenStream {
    if is_builtin_global_library(path) {
        return quote! { #path::STEP_LIBRARY };
    }
    let mut parent = path.clone();
    let Some(last) = parent.segments.pop() else {
        return quote! { compile_error!("step library path cannot be empty") };
    };
    let last = last.into_value();
    let marker = format_ident!("__RSTEST_BDD_STEP_LIBRARY_{}", last.ident);
    if parent.segments.is_empty() {
        quote! { #marker }
    } else {
        quote! { #parent::#marker }
    }
}

/// Recognize only the built-in global library for the resolved runtime crate.
fn is_builtin_global_library(path: &syn::Path) -> bool {
    let Ok(runtime) = syn::parse2::<syn::Path>(crate::codegen::rstest_bdd_path()) else {
        return false;
    };
    path.segments.len() == runtime.segments.len() + 1
        && path
            .segments
            .iter()
            .zip(&runtime.segments)
            .all(|(selected, expected)| selected.ident == expected.ident)
        && path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "global")
}

#[cfg(test)]
mod tests;
