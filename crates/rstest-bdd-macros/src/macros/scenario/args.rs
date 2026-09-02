//! Argument parsing for `#[scenario]` covering required `path`, mutually
//! exclusive `index`/`name` selectors, optional tag filters, and optional
//! harness adapter and attribute policy paths. Reports duplicates and
//! conflicts with combined `syn::Error`s.
use proc_macro2::Span;
use quote::{format_ident, quote};
use syn::{
    LitInt,
    LitStr,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    token::Comma,
};

/// Internal data used by the macros implementation.
pub(super) struct ScenarioArgs {
    /// Stores the internal `path` value.
    pub(super) path: LitStr,
    /// Stores the internal `selector` value.
    pub(super) selector: Option<ScenarioSelector>,
    /// Stores the internal `tag_filter` value.
    pub(super) tag_filter: Option<LitStr>,
    /// Stores the internal `harness` value.
    pub(super) harness: Option<syn::Path>,
    /// Stores the internal `attributes` value.
    pub(super) attributes: Option<syn::Path>,
    /// Explicitly selected step libraries.
    pub(super) libraries: Option<Vec<syn::Path>>,
}

/// Documents the internal `ScenarioSelector` item.
pub(super) enum ScenarioSelector {
    /// Represents the internal validation outcome.
    Index {
        /// Zero-based scenario index.
        value: usize,
        /// Source span of the index argument.
        span: Span,
    },
    /// Represents the internal validation outcome.
    Name {
        /// Scenario name selected by the argument.
        value: String,
        /// Source span of the name argument.
        span: Span,
    },
}

/// Documents the internal `ScenarioArg` item.
enum ScenarioArg {
    /// Represents the internal validation outcome.
    Path(LitStr),
    /// Represents the internal validation outcome.
    Index(LitInt),
    /// Represents the internal validation outcome.
    Name(LitStr),
    /// Represents the internal validation outcome.
    Tags(LitStr),
    /// Represents the internal validation outcome.
    Harness(syn::Path),
    /// Represents the internal validation outcome.
    Attributes(syn::Path),
    /// Closed list of selected step libraries.
    Libraries(Vec<syn::Path>),
}
impl Parse for ScenarioArg {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        if input.peek(LitStr) {
            Ok(Self::Path(input.parse()?))
        } else {
            Self::parse_named(input)
        }
    }
}
impl ScenarioArg {
    /// Parse a named `#[scenario]` argument.
    fn parse_named(input: ParseStream<'_>) -> syn::Result<Self> {
        let ident: syn::Ident = input.parse()?;
        input.parse::<syn::token::Eq>()?;
        match ident.to_string().as_str() {
            "path" => Ok(Self::Path(input.parse()?)),
            "index" => Ok(Self::Index(input.parse()?)),
            "name" => Ok(Self::Name(input.parse()?)),
            "tags" => Ok(Self::Tags(input.parse()?)),
            "harness" => Ok(Self::Harness(input.parse()?)),
            "attributes" => Ok(Self::Attributes(input.parse()?)),
            "libraries" => Ok(Self::Libraries(parse_libraries(input)?)),
            _ => Err(input.error(
                "expected `path`, `index`, `name`, `tags`, `harness`, `attributes`, or `libraries`",
            )),
        }
    }
}
/// Parse the closed list supplied to `libraries = [path, ...]`.
fn parse_libraries(input: ParseStream<'_>) -> syn::Result<Vec<syn::Path>> {
    let content;
    syn::bracketed!(content in input);
    Ok(Punctuated::<syn::Path, Comma>::parse_terminated(&content)?
        .into_iter()
        .collect())
}
impl Parse for ScenarioArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let args = Punctuated::<ScenarioArg, Comma>::parse_terminated(input)?;
        let mut path = None;
        let mut selector = None;
        let mut tag_filter = None;
        let mut harness = None;
        let mut attributes = None;
        let mut libraries = None;

        for arg in args {
            match arg {
                ScenarioArg::Path(lit) => set_unique_field(&mut path, lit, "path", input)?,
                ScenarioArg::Index(i) => set_selector_index(&mut selector, &i)?,
                ScenarioArg::Name(lit) => set_selector_name(&mut selector, &lit)?,
                ScenarioArg::Tags(lit) => set_unique_field(&mut tag_filter, lit, "tags", input)?,
                ScenarioArg::Harness(p) => {
                    set_unique_field(&mut harness, p, "harness", input)?;
                }
                ScenarioArg::Attributes(p) => {
                    set_unique_field(&mut attributes, p, "attributes", input)?;
                }
                ScenarioArg::Libraries(paths) => {
                    validate_unique_libraries(&paths)?;
                    set_unique_field(&mut libraries, paths, "libraries", input)?;
                }
            }
        }

        let path = path.ok_or_else(|| input.error("`path` argument is required"))?;

        Ok(Self {
            path,
            selector,
            tag_filter,
            harness,
            attributes,
            libraries,
        })
    }
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
///
/// These names are intentionally used only for definitions visible to the
/// current macro process. Cross-crate and out-of-line definitions remain
/// runtime-authoritative.
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
    let marker = format_ident!("__RSTEST_BDD_STEP_LIBRARY_{}", last.into_value().ident);
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
/// Assign `value` to `slot` if empty, or return a duplicate-argument error.
fn set_unique_field<T>(
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
/// Generic helper to set a selector after checking for conflicts.
fn set_selector<F>(
    selector: &mut Option<ScenarioSelector>,
    kind: SelectorKind,
    span: Span,
    build: F,
) -> syn::Result<()>
where
    F: FnOnce() -> syn::Result<ScenarioSelector>,
{
    if let Some(existing) = selector {
        return Err(selector_conflict_error(existing, kind, span));
    }
    *selector = Some(build()?);
    Ok(())
}
/// Set the scenario selector to an index, rejecting conflicts with an existing selector.
fn set_selector_index(selector: &mut Option<ScenarioSelector>, i: &LitInt) -> syn::Result<()> {
    set_selector(selector, SelectorKind::Index, i.span(), || {
        Ok(ScenarioSelector::Index {
            value: i.base10_parse()?,
            span: i.span(),
        })
    })
}
/// Set the scenario selector to a name, rejecting conflicts with an existing selector.
fn set_selector_name(selector: &mut Option<ScenarioSelector>, lit: &LitStr) -> syn::Result<()> {
    set_selector(selector, SelectorKind::Name, lit.span(), || {
        Ok(ScenarioSelector::Name {
            value: lit.value(),
            span: lit.span(),
        })
    })
}
/// Documents the internal `SelectorKind` item.
enum SelectorKind {
    /// Represents the internal validation outcome.
    Index,
    /// Represents the internal validation outcome.
    Name,
}
/// Provides the internal `selector_conflict_error` operation.
fn selector_conflict_error(
    existing: &ScenarioSelector,
    new_kind: SelectorKind,
    new_span: Span,
) -> syn::Error {
    match (existing, new_kind) {
        (ScenarioSelector::Index { .. }, SelectorKind::Index) => {
            syn::Error::new(new_span, "duplicate `index` argument")
        }
        (ScenarioSelector::Name { .. }, SelectorKind::Name) => {
            syn::Error::new(new_span, "duplicate `name` argument")
        }
        (ScenarioSelector::Index { span, .. }, SelectorKind::Name) => {
            let mut err = syn::Error::new(
                new_span,
                "`name` cannot be combined with `index`; choose one selector",
            );
            err.combine(syn::Error::new(
                *span,
                "`index` cannot be combined with `name`",
            ));
            err
        }
        (ScenarioSelector::Name { span, .. }, SelectorKind::Index) => {
            let mut err = syn::Error::new(new_span, "`index` cannot be combined with `name`");
            err.combine(syn::Error::new(
                *span,
                "`name` cannot be combined with `index`; choose one selector",
            ));
            err
        }
    }
}
#[cfg(test)]
mod tests;
