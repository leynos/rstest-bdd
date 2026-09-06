//! Indexes closed step-library scopes selected by Rust scenario bindings.

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use syn::{
    LitStr,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    token::Comma,
};

use super::super::{IndexedScenarioBinding, ScenarioBindingTarget};

mod diagnostics;
#[cfg(test)]
mod tests;

use diagnostics::BindingIndexFailure;
pub(crate) use diagnostics::ScenarioBindingIndexDiagnostic;

/// Binding kind determines whether the path names one feature or a directory.
#[derive(Clone, Copy)]
enum BindingKind {
    /// `#[scenario]` selects one feature file.
    Feature,
    /// `scenarios!` selects every feature under one directory.
    Directory,
}

/// Arguments relevant to language-server scope selection.
#[derive(Default)]
struct BindingArguments {
    /// Feature file or directory literal.
    path: Option<LitStr>,
    /// Explicit closed step-library list.
    libraries: Option<Vec<syn::Path>>,
}

/// One parsed argument, including ignored arguments accepted by the macros.
enum BindingArgument {
    /// Positional or named feature path.
    Path(LitStr),
    /// Closed step-library list.
    Libraries(Vec<syn::Path>),
    /// An argument that does not affect scope selection.
    Ignored,
}

/// Pure scenario-binding indexing output for one Rust source file.
pub(super) struct ScenarioBindingIndex {
    /// Successfully indexed bindings.
    pub(super) bindings: Vec<IndexedScenarioBinding>,
    /// Recoverable indexing diagnostics.
    pub(super) diagnostics: Vec<ScenarioBindingIndexDiagnostic>,
}

/// Validated binding arguments required by scope indexing.
struct ParsedBindingArguments {
    /// Feature file or directory selected by the binding.
    path: LitStr,
    /// Explicit closed library scope, when present.
    libraries: Option<Vec<syn::Path>>,
}

impl Parse for BindingArgument {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        if input.peek(LitStr) {
            return Ok(Self::Path(input.parse()?));
        }

        let name: syn::Ident = input.parse()?;
        input.parse::<syn::Token![=]>()?;
        match name.to_string().as_str() {
            "path" | "dir" => Ok(Self::Path(input.parse()?)),
            "libraries" => parse_libraries(input),
            "fixtures" => {
                let content;
                syn::bracketed!(content in input);
                let _: proc_macro2::TokenStream = content.parse()?;
                Ok(Self::Ignored)
            }
            "index" => {
                let _: syn::LitInt = input.parse()?;
                Ok(Self::Ignored)
            }
            "name" | "tags" | "runtime" => {
                let _: LitStr = input.parse()?;
                Ok(Self::Ignored)
            }
            "harness" | "attributes" => {
                let _: syn::Path = input.parse()?;
                Ok(Self::Ignored)
            }
            _ => {
                let _: syn::Expr = input.parse()?;
                Ok(Self::Ignored)
            }
        }
    }
}

/// Parse a `libraries = [path, ...]` argument.
fn parse_libraries(input: ParseStream<'_>) -> syn::Result<BindingArgument> {
    let content;
    syn::bracketed!(content in input);
    let paths = Punctuated::<syn::Path, Comma>::parse_terminated(&content)?;
    Ok(BindingArgument::Libraries(paths.into_iter().collect()))
}

impl Parse for BindingArguments {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let arguments = Punctuated::<BindingArgument, Comma>::parse_terminated(input)?;
        let mut parsed = Self::default();
        for argument in arguments {
            match argument {
                BindingArgument::Path(path) => parsed.path = Some(path),
                BindingArgument::Libraries(libraries) => parsed.libraries = Some(libraries),
                BindingArgument::Ignored => {}
            }
        }
        Ok(parsed)
    }
}

/// Collect scenario bindings from one parsed Rust file.
pub(super) fn index_scenario_bindings(
    file: &syn::File,
    source_path: &Path,
) -> ScenarioBindingIndex {
    let module_paths = collect_module_paths(&file.items);
    let mut bindings = Vec::new();
    let mut diagnostics = Vec::new();
    collect_bindings(
        &file.items,
        &[],
        &module_paths,
        source_path,
        &mut bindings,
        &mut diagnostics,
    );
    ScenarioBindingIndex {
        bindings,
        diagnostics,
    }
}

/// Collect paths of inline modules available to binding path resolution.
fn collect_module_paths(items: &[syn::Item]) -> HashSet<Vec<String>> {
    let mut paths = HashSet::new();
    collect_module_paths_inner(items, &mut Vec::new(), &mut paths);
    paths
}

/// Recursively record all inline module paths.
fn collect_module_paths_inner(
    items: &[syn::Item],
    module_path: &mut Vec<String>,
    paths: &mut HashSet<Vec<String>>,
) {
    for item in items {
        let syn::Item::Mod(module) = item else {
            continue;
        };
        let Some((_, nested)) = &module.content else {
            continue;
        };
        module_path.push(module.ident.to_string());
        paths.insert(module_path.clone());
        collect_module_paths_inner(nested, module_path, paths);
        module_path.pop();
    }
}

/// Traverse inline Rust modules and collect scenario attributes and macros.
fn collect_bindings(
    items: &[syn::Item],
    module_path: &[String],
    module_paths: &HashSet<Vec<String>>,
    source_path: &Path,
    bindings: &mut Vec<IndexedScenarioBinding>,
    diagnostics: &mut Vec<ScenarioBindingIndexDiagnostic>,
) {
    for item in items {
        match item {
            syn::Item::Fn(function) => {
                collect_scenario_attribute(
                    function,
                    module_path,
                    module_paths,
                    source_path,
                    bindings,
                    diagnostics,
                );
            }
            syn::Item::Macro(item_macro)
                if macro_name(&item_macro.mac).as_deref() == Some("scenarios") =>
            {
                collect_binding(
                    &item_macro.mac.tokens,
                    BindingKind::Directory,
                    module_path,
                    module_paths,
                    source_path,
                    bindings,
                    diagnostics,
                );
            }
            syn::Item::Mod(module) => {
                if let Some((_, nested)) = &module.content {
                    let mut nested_path = module_path.to_vec();
                    nested_path.push(module.ident.to_string());
                    collect_bindings(
                        nested,
                        &nested_path,
                        module_paths,
                        source_path,
                        bindings,
                        diagnostics,
                    );
                }
            }
            _ => {}
        }
    }
}

/// Collect a `#[scenario(...)]` attribute from one function.
fn collect_scenario_attribute(
    function: &syn::ItemFn,
    module_path: &[String],
    module_paths: &HashSet<Vec<String>>,
    source_path: &Path,
    bindings: &mut Vec<IndexedScenarioBinding>,
    diagnostics: &mut Vec<ScenarioBindingIndexDiagnostic>,
) {
    for attribute in &function.attrs {
        if attribute
            .path()
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "scenario")
        {
            if let syn::Meta::List(list) = &attribute.meta {
                collect_binding(
                    &list.tokens,
                    BindingKind::Feature,
                    module_path,
                    module_paths,
                    source_path,
                    bindings,
                    diagnostics,
                );
            }
        }
    }
}

/// Return the final segment of a macro path.
fn macro_name(item_macro: &syn::Macro) -> Option<String> {
    item_macro
        .path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
}

/// Parse one binding and append it when it has a target path.
fn collect_binding(
    tokens: &proc_macro2::TokenStream,
    kind: BindingKind,
    module_path: &[String],
    module_paths: &HashSet<Vec<String>>,
    source_path: &Path,
    bindings: &mut Vec<IndexedScenarioBinding>,
    diagnostics: &mut Vec<ScenarioBindingIndexDiagnostic>,
) {
    let arguments = match parse_binding_arguments(tokens) {
        Ok(arguments) => arguments,
        Err(failure) => {
            diagnostics.push(ScenarioBindingIndexDiagnostic::new(
                source_path,
                tokens,
                failure,
            ));
            return;
        }
    };
    let target_path = PathBuf::from(arguments.path.value());
    let target = match kind {
        BindingKind::Feature => ScenarioBindingTarget::Feature(target_path),
        BindingKind::Directory => ScenarioBindingTarget::Directory(target_path),
    };
    let libraries = arguments.libraries.map_or_else(
        || vec![String::from("rstest_bdd::global")],
        |paths| {
            paths
                .iter()
                .map(|path| resolve_library_path(path, module_path, module_paths))
                .collect()
        },
    );
    bindings.push(IndexedScenarioBinding { target, libraries });
}

/// Parse one binding and require the target used by scope resolution.
fn parse_binding_arguments(
    tokens: &proc_macro2::TokenStream,
) -> Result<ParsedBindingArguments, BindingIndexFailure> {
    let arguments = syn::parse2::<BindingArguments>(tokens.clone())
        .map_err(|_| BindingIndexFailure::Malformed)?;
    let path = arguments.path.ok_or(BindingIndexFailure::MissingPath)?;
    Ok(ParsedBindingArguments {
        path,
        libraries: arguments.libraries,
    })
}

/// Resolve a selected library as an ordinary path from its enclosing module.
fn resolve_library_path(
    path: &syn::Path,
    module_path: &[String],
    module_paths: &HashSet<Vec<String>>,
) -> String {
    let segments: Vec<_> = path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect();
    let mut resolved = path_prefix(path, &segments, module_path, module_paths);
    resolved.extend(path_suffix(&segments));
    resolved.join("::")
}

/// Select the lexical base for a Rust library path.
fn path_prefix(
    path: &syn::Path,
    segments: &[String],
    module_path: &[String],
    module_paths: &HashSet<Vec<String>>,
) -> Vec<String> {
    if path.leading_colon.is_some() || segments.first().is_some_and(|segment| segment == "crate") {
        return Vec::new();
    }
    if is_builtin_global_path(segments) {
        return Vec::new();
    }
    if segments.first().is_some_and(|segment| segment == "self") {
        return module_path.to_vec();
    }
    if segments.first().is_some_and(|segment| segment == "super") {
        let mut prefix = module_path.to_vec();
        for _ in segments.iter().take_while(|segment| *segment == "super") {
            prefix.pop();
        }
        return prefix;
    }
    let mut local_candidate = module_path.to_vec();
    if let Some(segment) = segments.first() {
        local_candidate.push(segment.clone());
    }
    if module_paths.contains(&local_candidate) {
        return module_path.to_vec();
    }
    Vec::new()
}

/// Return path segments after leading Rust-relative qualifiers.
fn path_suffix(segments: &[String]) -> impl Iterator<Item = String> + '_ {
    segments
        .iter()
        .skip_while(|segment| matches!(segment.as_str(), "crate" | "self" | "super"))
        .cloned()
}

/// Recognize the built-in global library when named through the runtime crate.
fn is_builtin_global_path(segments: &[String]) -> bool {
    matches!(segments, [runtime, global] if runtime == "rstest_bdd" && global == "global")
}
