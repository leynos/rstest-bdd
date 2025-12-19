//! Rust step definition indexing support.
//!
//! This module parses Rust source code with `syn` and extracts functions
//! annotated with the `rstest-bdd` step macros: `#[given]`, `#[when]`, and
//! `#[then]`.
//!
//! The indexer intentionally mirrors the macro behaviour:
//!
//! - Missing attribute arguments infer the pattern from the function name by
//!   replacing underscores with spaces.
//! - A string literal containing only whitespace also triggers inference.
//! - The literal empty string (`""`) registers an empty pattern and does not
//!   infer.
//! - A data table is expected when a parameter is named `datatable` or has a
//!   `#[datatable]` parameter attribute.
//! - A doc string is expected when a parameter is named `docstring` and its
//!   type resolves to `String` (either `String` or `std::string::String`).

use std::path::{Path, PathBuf};

use gherkin::StepType;
use quote::ToTokens;

use super::{
    IndexedStepDefinition, IndexedStepParameter, RustFunctionId, RustStepFileIndex,
    RustStepIndexError,
};

/// Parse and index a Rust source file from disk.
///
/// # Errors
///
/// Returns an error when the file cannot be read or parsed as Rust source.
///
/// # Examples
///
/// ```
/// use rstest_bdd_server::indexing::index_rust_file;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let path = std::env::temp_dir().join(format!(
///     "rstest-bdd-server-index-rust-file-{}-{}.rs",
///     std::process::id(),
///     std::time::SystemTime::now()
///         .duration_since(std::time::UNIX_EPOCH)?
///         .as_nanos(),
/// ));
/// std::fs::write(&path, "#[given(\"a message\")]\nfn a_message() {}\n")?;
///
/// let index = index_rust_file(&path)?;
/// assert_eq!(index.path, path);
///
/// # std::fs::remove_file(&index.path).ok();
/// # Ok(())
/// # }
/// ```
pub fn index_rust_file(path: &Path) -> Result<RustStepFileIndex, RustStepIndexError> {
    let source = std::fs::read_to_string(path)?;
    index_rust_source(path.to_path_buf(), &source)
}

/// Parse and index Rust step definitions from source text.
///
/// This is intended for language-server integrations that receive saved text
/// from the client and want to avoid a race with filesystem writes.
///
/// # Errors
///
/// Returns an error when the source cannot be parsed by `syn`.
///
/// # Examples
///
/// ```
/// use std::path::PathBuf;
///
/// use rstest_bdd_server::indexing::index_rust_source;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let source = "#[when]\nfn do_the_thing() {}\n";
/// let index = index_rust_source(PathBuf::from("steps.rs"), source)?;
/// assert_eq!(index.step_definitions.len(), 1);
///
/// let step = index.step_definitions.first().expect("indexed step");
/// assert_eq!(step.pattern, "do the thing");
/// # Ok(())
/// # }
/// ```
pub fn index_rust_source(
    path: PathBuf,
    source: &str,
) -> Result<RustStepFileIndex, RustStepIndexError> {
    let file = syn::parse_file(source)?;
    let mut step_definitions = Vec::new();
    let mut module_path = Vec::new();
    collect_step_definitions(&file.items, &mut module_path, &mut step_definitions)?;

    Ok(RustStepFileIndex {
        path,
        step_definitions,
    })
}

fn collect_step_definitions(
    items: &[syn::Item],
    module_path: &mut Vec<String>,
    out: &mut Vec<IndexedStepDefinition>,
) -> Result<(), RustStepIndexError> {
    for item in items {
        match item {
            syn::Item::Fn(item_fn) => {
                if let Some(step) = index_step_function(item_fn, module_path)? {
                    out.push(step);
                }
            }
            syn::Item::Mod(item_mod) => {
                let Some((_, items)) = item_mod.content.as_ref() else {
                    continue;
                };
                module_path.push(item_mod.ident.to_string());
                collect_step_definitions(items, module_path, out)?;
                module_path.pop();
            }
            _ => {}
        }
    }
    Ok(())
}

fn index_step_function(
    item_fn: &syn::ItemFn,
    module_path: &[String],
) -> Result<Option<IndexedStepDefinition>, RustStepIndexError> {
    let mut step_attribute: Option<StepAttribute<'_>> = None;
    for attr in &item_fn.attrs {
        let Some(attr_keyword) = step_attribute_keyword(attr) else {
            continue;
        };

        if step_attribute.is_some() {
            return Err(RustStepIndexError::MultipleStepAttributes {
                function: item_fn.sig.ident.to_string(),
            });
        }

        step_attribute = Some(StepAttribute {
            keyword: attr_keyword.step_type,
            attr,
        });
    }

    let Some(step_attribute) = step_attribute else {
        return Ok(None);
    };

    let (pattern, pattern_inferred) = parse_step_pattern(step_attribute.attr, &item_fn.sig.ident)?;

    let mut parameters = Vec::new();
    for input in &item_fn.sig.inputs {
        match input {
            syn::FnArg::Receiver(_) => parameters.push(IndexedStepParameter {
                name: Some("self".to_string()),
                ty: "Self".to_string(),
                is_datatable: false,
                is_docstring: false,
            }),
            syn::FnArg::Typed(pat_type) => {
                let name = param_name(&pat_type.pat);
                let ty = pat_type.ty.to_token_stream().to_string();
                let is_datatable = parameter_is_datatable(pat_type, name.as_deref());
                let is_docstring = parameter_is_docstring(name.as_deref(), &pat_type.ty);
                parameters.push(IndexedStepParameter {
                    name,
                    ty,
                    is_datatable,
                    is_docstring,
                });
            }
        }
    }

    let expects_table = parameters.iter().any(|param| param.is_datatable);
    let expects_docstring = parameters.iter().any(|param| param.is_docstring);

    Ok(Some(IndexedStepDefinition {
        keyword: step_attribute.keyword,
        pattern,
        pattern_inferred,
        function: RustFunctionId {
            module_path: module_path.to_vec(),
            name: item_fn.sig.ident.to_string(),
        },
        parameters,
        expects_table,
        expects_docstring,
    }))
}

struct AttributeKeyword {
    name: &'static str,
    step_type: StepType,
}

fn step_attribute_keyword(attr: &syn::Attribute) -> Option<AttributeKeyword> {
    let ident = attr.path().segments.last()?.ident.to_string();
    match ident.as_str() {
        "given" => Some(AttributeKeyword {
            name: "given",
            step_type: StepType::Given,
        }),
        "when" => Some(AttributeKeyword {
            name: "when",
            step_type: StepType::When,
        }),
        "then" => Some(AttributeKeyword {
            name: "then",
            step_type: StepType::Then,
        }),
        _ => None,
    }
}

struct StepAttribute<'a> {
    keyword: StepType,
    attr: &'a syn::Attribute,
}

fn parse_step_pattern(
    attr: &syn::Attribute,
    function_ident: &syn::Ident,
) -> Result<(String, bool), RustStepIndexError> {
    let attribute = step_attribute_keyword(attr).map_or("step", |kw| kw.name);
    match &attr.meta {
        syn::Meta::Path(_) => Ok((infer_pattern(function_ident), true)),
        syn::Meta::List(meta_list) => {
            if meta_list.tokens.is_empty() {
                return Ok((infer_pattern(function_ident), true));
            }
            let pattern_lit = attr.parse_args::<syn::LitStr>().map_err(|err| {
                RustStepIndexError::InvalidStepAttributeArguments {
                    function: function_ident.to_string(),
                    attribute,
                    message: err.to_string(),
                }
            })?;
            Ok(interpret_pattern_literal(
                function_ident,
                pattern_lit.value(),
            ))
        }
        syn::Meta::NameValue(name_value) => {
            let syn::Expr::Lit(expr_lit) = &name_value.value else {
                return Err(RustStepIndexError::InvalidStepAttributeArguments {
                    function: function_ident.to_string(),
                    attribute,
                    message: "expected string literal value".to_string(),
                });
            };
            let syn::Lit::Str(lit) = &expr_lit.lit else {
                return Err(RustStepIndexError::InvalidStepAttributeArguments {
                    function: function_ident.to_string(),
                    attribute,
                    message: "expected string literal value".to_string(),
                });
            };
            Ok(interpret_pattern_literal(function_ident, lit.value()))
        }
    }
}

fn interpret_pattern_literal(function_ident: &syn::Ident, raw: String) -> (String, bool) {
    if raw.is_empty() {
        return (raw, false);
    }
    if raw.trim().is_empty() {
        return (infer_pattern(function_ident), true);
    }
    (raw, false)
}

fn infer_pattern(function_ident: &syn::Ident) -> String {
    function_ident.to_string().replace('_', " ")
}

fn param_name(pat: &syn::Pat) -> Option<String> {
    match pat {
        syn::Pat::Ident(pat_ident) => Some(pat_ident.ident.to_string()),
        _ => None,
    }
}

fn parameter_is_datatable(pat_type: &syn::PatType, name: Option<&str>) -> bool {
    if name.is_some_and(|value| value == "datatable") {
        return true;
    }

    pat_type.attrs.iter().any(|attr| {
        attr.path()
            .segments
            .last()
            .is_some_and(|seg| seg.ident == "datatable")
    })
}

fn parameter_is_docstring(name: Option<&str>, ty: &syn::Type) -> bool {
    if name.is_none_or(|value| value != "docstring") {
        return false;
    }
    type_is_string(ty)
}

fn type_is_string(ty: &syn::Type) -> bool {
    let syn::Type::Path(type_path) = ty else {
        return false;
    };

    type_path
        .path
        .segments
        .last()
        .is_some_and(|seg| seg.ident == "String")
}

#[cfg(test)]
mod tests;
