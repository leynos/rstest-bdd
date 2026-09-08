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
mod path_resolution;
#[cfg(test)]
mod tests;

use diagnostics::BindingIndexFailure;
pub(crate) use diagnostics::ScenarioBindingIndexDiagnostic;
use path_resolution::resolve_library_path;

/// Binding kind determines whether the path names one feature or a directory.
#[derive(Clone, Copy)]
enum BindingKind {
    /// `#[scenario]` selects one feature file.
    Feature,
    /// `scenarios!` selects every feature under one directory.
    Directory,
}

/// Mutable state shared while indexing bindings from one Rust source file.
struct BindingCollector<'a> {
    /// Inline modules available for lexical library-path resolution.
    module_paths: &'a HashSet<Vec<String>>,
    /// Rust source file that owns all collected bindings.
    source_path: &'a Path,
    /// Successfully indexed scenario bindings in lexical source order.
    bindings: &'a mut Vec<IndexedScenarioBinding>,
    /// Recoverable diagnostics encountered while indexing bindings.
    diagnostics: &'a mut Vec<ScenarioBindingIndexDiagnostic>,
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
    {
        let mut collector = BindingCollector {
            module_paths: &module_paths,
            source_path,
            bindings: &mut bindings,
            diagnostics: &mut diagnostics,
        };
        collector.collect_bindings(&file.items, &[]);
    }
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

/// Return the final segment of a macro path.
fn macro_name(item_macro: &syn::Macro) -> Option<String> {
    item_macro
        .path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
}

impl BindingCollector<'_> {
    /// Traverse inline Rust modules and collect scenario attributes and macros.
    fn collect_bindings(&mut self, items: &[syn::Item], module_path: &[String]) {
        for item in items {
            match item {
                syn::Item::Fn(function) => self.collect_scenario_attribute(function, module_path),
                syn::Item::Macro(item_macro)
                    if macro_name(&item_macro.mac).as_deref() == Some("scenarios") =>
                {
                    self.collect_binding(
                        &item_macro.mac.tokens,
                        BindingKind::Directory,
                        module_path,
                    );
                }
                syn::Item::Mod(module) => {
                    if let Some((_, nested)) = &module.content {
                        let mut nested_path = module_path.to_vec();
                        nested_path.push(module.ident.to_string());
                        self.collect_bindings(nested, &nested_path);
                    }
                }
                _ => {}
            }
        }
    }

    /// Collect a `#[scenario(...)]` attribute from one function.
    fn collect_scenario_attribute(&mut self, function: &syn::ItemFn, module_path: &[String]) {
        for attribute in &function.attrs {
            let is_scenario = attribute
                .path()
                .segments
                .last()
                .is_some_and(|segment| segment.ident == "scenario");
            if is_scenario && let syn::Meta::List(list) = &attribute.meta {
                self.collect_binding(&list.tokens, BindingKind::Feature, module_path);
            }
        }
    }

    /// Parse one binding and append it when it has a target path.
    fn collect_binding(
        &mut self,
        tokens: &proc_macro2::TokenStream,
        kind: BindingKind,
        module_path: &[String],
    ) {
        let arguments = match parse_binding_arguments(tokens) {
            Ok(arguments) => arguments,
            Err(failure) => {
                self.diagnostics.push(ScenarioBindingIndexDiagnostic::new(
                    self.source_path,
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
                    .map(|path| resolve_library_path(path, module_path, self.module_paths))
                    .collect()
            },
        );
        self.bindings
            .push(IndexedScenarioBinding { target, libraries });
    }
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
