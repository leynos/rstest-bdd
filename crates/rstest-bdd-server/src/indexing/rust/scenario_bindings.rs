//! Indexes closed step-library scopes selected by Rust scenario bindings.

use std::path::PathBuf;

use syn::{
    LitStr,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    token::Comma,
};

use super::super::{IndexedScenarioBinding, ScenarioBindingTarget};

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
pub(super) fn index_scenario_bindings(file: &syn::File) -> Vec<IndexedScenarioBinding> {
    let mut bindings = Vec::new();
    collect_bindings(&file.items, &mut bindings);
    bindings
}

/// Traverse inline Rust modules and collect scenario attributes and macros.
fn collect_bindings(items: &[syn::Item], bindings: &mut Vec<IndexedScenarioBinding>) {
    for item in items {
        match item {
            syn::Item::Fn(function) => collect_scenario_attribute(function, bindings),
            syn::Item::Macro(item_macro)
                if macro_name(&item_macro.mac).as_deref() == Some("scenarios") =>
            {
                collect_binding(&item_macro.mac.tokens, BindingKind::Directory, bindings);
            }
            syn::Item::Mod(module) => {
                if let Some((_, nested)) = &module.content {
                    collect_bindings(nested, bindings);
                }
            }
            _ => {}
        }
    }
}

/// Collect a `#[scenario(...)]` attribute from one function.
fn collect_scenario_attribute(function: &syn::ItemFn, bindings: &mut Vec<IndexedScenarioBinding>) {
    for attribute in &function.attrs {
        if attribute
            .path()
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "scenario")
        {
            if let syn::Meta::List(list) = &attribute.meta {
                collect_binding(&list.tokens, BindingKind::Feature, bindings);
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
    bindings: &mut Vec<IndexedScenarioBinding>,
) {
    let Ok(arguments) = syn::parse2::<BindingArguments>(tokens.clone()) else {
        return;
    };
    let Some(path) = arguments.path else {
        return;
    };
    let target_path = PathBuf::from(path.value());
    let target = match kind {
        BindingKind::Feature => ScenarioBindingTarget::Feature(target_path),
        BindingKind::Directory => ScenarioBindingTarget::Directory(target_path),
    };
    let libraries = arguments.libraries.map_or_else(
        || vec![String::from("rstest_bdd::global")],
        |paths| paths.iter().map(render_library_path).collect(),
    );
    bindings.push(IndexedScenarioBinding { target, libraries });
}

/// Render a Rust library path using the macro's local validation identity.
fn render_library_path(path: &syn::Path) -> String {
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
}

#[cfg(test)]
mod tests {
    //! Unit tests for Rust scenario-binding indexing.

    use super::*;

    #[test]
    fn indexes_scenario_and_scenarios_library_scopes() {
        let file = syn::parse_file(concat!(
            "#[scenario(path = \"tests/features/account.feature\", libraries = [common, \
             accounts])]\n",
            "fn account() {}\n",
            "scenarios!(dir = \"tests/features/files\", libraries = [crate::filesystem]);\n",
        ))
        .expect("Rust source");

        let bindings = index_scenario_bindings(&file);

        assert_eq!(bindings.len(), 2);
        let account_binding = bindings.first().expect("account scenario binding");
        let filesystem_binding = bindings.get(1).expect("filesystem scenarios binding");
        assert_eq!(account_binding.libraries, ["common", "accounts"]);
        assert_eq!(filesystem_binding.libraries, ["filesystem"]);
        assert!(matches!(
            &account_binding.target,
            ScenarioBindingTarget::Feature(path)
                if path == &PathBuf::from("tests/features/account.feature")
        ));
        assert!(matches!(
            &filesystem_binding.target,
            ScenarioBindingTarget::Directory(path)
                if path == &PathBuf::from("tests/features/files")
        ));
    }

    #[test]
    fn defaults_unscoped_scenarios_to_the_global_library() {
        let file =
            syn::parse_file("#[scenario(\"test.feature\")]\nfn test() {}\n").expect("Rust source");

        let bindings = index_scenario_bindings(&file);

        assert_eq!(
            bindings.first().expect("global scenario binding").libraries,
            ["rstest_bdd::global"]
        );
    }
}
