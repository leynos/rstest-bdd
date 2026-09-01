//! Rust step definition indexing support.
//!
//! This module parses Rust source code with `syn` and extracts functions
//! annotated with the `rstest-bdd` step macros: `#[given]`, `#[when]`, and
//! `#[then]`.
//!
//! The indexer intentionally mirrors the macro behaviour:
//!
//! - Missing attribute arguments infer the pattern from the function name by replacing underscores
//!   with spaces.
//! - A string literal containing only whitespace also triggers inference.
//! - The literal empty string (`""`) registers an empty pattern and does not infer.
//! - A data table is expected when a parameter is named `datatable` or has a `#[datatable]`
//!   parameter attribute.
//! - A doc string is expected when a parameter is named `docstring` and its type resolves to
//!   `String` (either `String` or `std::string::String`).

use std::path::PathBuf;

use gherkin::StepType;
use syn::spanned::Spanned;

use super::{
    IndexedStepDefinition,
    IndexedStepParameter,
    RustAttributeSpan,
    RustFunctionId,
    RustStepFileIndex,
    RustStepIndexDiagnostic,
    RustStepIndexError,
    RustStepIndexResult,
};

mod entry;
mod params;
mod scenario_bindings;
mod type_render;

pub(crate) use entry::{
    RustSourceIndexResult,
    index_rust_file_with_bindings,
    index_rust_source_with_bindings,
};
pub use entry::{index_rust_file, index_rust_source};
use params::parse_function_parameters;
use scenario_bindings::index_scenario_bindings;

/// Build public step metadata and internal scenario scopes in one syntax-tree traversal.
fn parse_rust_source_with_bindings(
    path: PathBuf,
    source: &str,
) -> Result<RustSourceIndexResult, RustStepIndexError> {
    let file = syn::parse_file(source)?;
    let scenario_bindings = index_scenario_bindings(&file);
    let mut collector = StepDefinitionCollector {
        source,
        module_path: Vec::new(),
        library_path: None,
        step_definitions: Vec::new(),
        diagnostics: Vec::new(),
    };
    collector.collect_step_definitions(&file.items);
    let StepDefinitionCollector {
        step_definitions,
        diagnostics,
        ..
    } = collector;

    Ok(RustSourceIndexResult {
        steps: RustStepIndexResult {
            index: RustStepFileIndex {
                path,
                step_definitions,
            },
            diagnostics,
        },
        scenario_bindings,
    })
}

/// Accumulates indexing output for one Rust source traversal.
struct StepDefinitionCollector<'a> {
    /// Complete Rust source being traversed.
    source: &'a str,
    /// Module path of the item currently being traversed.
    module_path: Vec<String>,
    /// Nearest `#[step_library]` module enclosing the current item.
    library_path: Option<Vec<String>>,
    /// Step definitions discovered in the source.
    step_definitions: Vec<IndexedStepDefinition>,
    /// Recoverable diagnostics discovered during indexing.
    diagnostics: Vec<RustStepIndexDiagnostic>,
}

impl StepDefinitionCollector<'_> {
    /// Traverse Rust items and collect nested step definitions.
    fn collect_step_definitions(&mut self, items: &[syn::Item]) {
        for item in items {
            match item {
                syn::Item::Fn(item_fn) => {
                    match index_step_function(
                        item_fn,
                        self.source,
                        &self.module_path,
                        self.library_path.as_deref(),
                    ) {
                        Ok(Some(step)) => self.step_definitions.push(step),
                        Ok(None) => {}
                        Err(diagnostic) => self.diagnostics.push(diagnostic),
                    }
                }
                syn::Item::Mod(item_mod) => {
                    let Some((_, items)) = item_mod.content.as_ref() else {
                        continue;
                    };
                    self.module_path.push(item_mod.ident.to_string());
                    let previous_library = self.library_path.clone();
                    if is_step_library(item_mod) {
                        self.library_path = Some(self.module_path.clone());
                    }
                    self.collect_step_definitions(items);
                    self.library_path = previous_library;
                    self.module_path.pop();
                }
                _ => {}
            }
        }
    }
}

/// Return whether a module establishes a lexical step-library boundary.
fn is_step_library(item_mod: &syn::ItemMod) -> bool {
    item_mod
        .attrs
        .iter()
        .any(|attribute| attribute.path().is_ident("step_library"))
}
/// Find and validate the step attribute on a function.
///
/// Returns `None` if no step attribute is found, or `Some(StepAttribute)` if
/// exactly one is present. Returns an error if multiple step attributes exist.
fn find_step_attribute(
    item_fn: &syn::ItemFn,
) -> Result<Option<StepAttribute<'_>>, RustStepIndexDiagnostic> {
    let mut step_attribute: Option<StepAttribute<'_>> = None;

    for attr in &item_fn.attrs {
        let Some(attr_keyword) = step_attribute_keyword(attr) else {
            continue;
        };

        if step_attribute.is_some() {
            return Err(RustStepIndexDiagnostic::MultipleStepAttributes {
                function: item_fn.sig.ident.to_string(),
            });
        }

        step_attribute = Some(StepAttribute {
            keyword: attr_keyword.step_type,
            attribute: attr_keyword.name,
            attr,
        });
    }

    Ok(step_attribute)
}
/// Index one function when it carries a recognized step attribute.
fn index_step_function(
    item_fn: &syn::ItemFn,
    source: &str,
    module_path: &[String],
    library_path: Option<&[String]>,
) -> Result<Option<IndexedStepDefinition>, RustStepIndexDiagnostic> {
    let Some(step_attribute) = find_step_attribute(item_fn)? else {
        return Ok(None);
    };

    let (pattern, pattern_inferred) = parse_step_pattern(
        step_attribute.attr,
        &item_fn.sig.ident,
        step_attribute.attribute,
    )?;

    let parameters = parse_function_parameters(&item_fn.sig.inputs);
    let expects_table = parameters.iter().any(|param| param.is_datatable);
    let expects_docstring = parameters.iter().any(|param| param.is_docstring);

    // Extract span from the step attribute (syn uses 1-based line numbers).
    // Line/column numbers in practice will never exceed u32::MAX, so truncation is safe.
    let attribute_span = extract_attribute_span(step_attribute.attr, &item_fn.sig, source);

    Ok(Some(IndexedStepDefinition {
        library: library_path.map_or_else(
            || String::from("rstest_bdd::global"),
            |path| path.join("::"),
        ),
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
        attribute_span,
    }))
}

/// Step keyword and attribute name extracted from a Rust attribute.
struct AttributeKeyword {
    /// Attribute name without the `#[]` delimiters.
    name: &'static str,
    /// Gherkin keyword represented by the attribute.
    step_type: StepType,
}

/// Recognize a `given`, `when`, or `then` step attribute.
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

/// Borrowed step attribute details used during indexing.
struct StepAttribute<'a> {
    /// Gherkin keyword represented by the attribute.
    keyword: StepType,
    /// Attribute name used in diagnostics.
    attribute: &'static str,
    /// Original syntax node for the attribute.
    attr: &'a syn::Attribute,
}

/// Extract the span of a step attribute and function line as 0-based positions.
///
/// Converts `syn`'s 1-based line numbers to 0-based for LSP compatibility.
/// Byte column offsets from `syn` are converted to UTF-16 code units as required
/// by the LSP specification.
fn extract_attribute_span(
    attr: &syn::Attribute,
    fn_sig: &syn::Signature,
    source: &str,
) -> RustAttributeSpan {
    use crate::util::byte_col_to_utf16_col;
    let span = attr.span();
    let start = span.start();
    let end = span.end();
    let fn_line =
        u32::try_from(fn_sig.fn_token.span().start().line.saturating_sub(1)).unwrap_or(u32::MAX);
    let start_line_0 = start.line.saturating_sub(1);
    let end_line_0 = end.line.saturating_sub(1);
    let start_col_utf16 = byte_col_to_utf16_col(source, start_line_0, start.column);
    let end_col_utf16 = byte_col_to_utf16_col(source, end_line_0, end.column);
    RustAttributeSpan {
        start_line: u32::try_from(start_line_0).unwrap_or(u32::MAX),
        start_column: start_col_utf16,
        end_line: u32::try_from(end_line_0).unwrap_or(u32::MAX),
        end_column: end_col_utf16,
        function_line: fn_line,
    }
}

/// Parse a step pattern, inferring it when the attribute has no value.
fn parse_step_pattern(
    attr: &syn::Attribute,
    function_ident: &syn::Ident,
    attribute: &'static str,
) -> Result<(String, bool), RustStepIndexDiagnostic> {
    match &attr.meta {
        syn::Meta::Path(_) => Ok((infer_pattern(function_ident), true)),
        syn::Meta::List(meta_list) => {
            if meta_list.tokens.is_empty() {
                return Ok((infer_pattern(function_ident), true));
            }
            let pattern_lit = attr.parse_args::<syn::LitStr>().map_err(|err| {
                RustStepIndexDiagnostic::InvalidStepAttributeArguments {
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
                return Err(RustStepIndexDiagnostic::InvalidStepAttributeArguments {
                    function: function_ident.to_string(),
                    attribute,
                    message: "expected string literal value".to_owned(),
                });
            };
            let syn::Lit::Str(lit) = &expr_lit.lit else {
                return Err(RustStepIndexDiagnostic::InvalidStepAttributeArguments {
                    function: function_ident.to_string(),
                    attribute,
                    message: "expected string literal value".to_owned(),
                });
            };
            Ok(interpret_pattern_literal(function_ident, lit.value()))
        }
    }
}

/// Interpret a literal pattern and report whether it was inferred.
fn interpret_pattern_literal(function_ident: &syn::Ident, raw: String) -> (String, bool) {
    if raw.is_empty() {
        return (raw, false);
    }
    if raw.trim().is_empty() {
        return (infer_pattern(function_ident), true);
    }
    (raw, false)
}

/// Infer a human-readable step pattern from a function identifier.
fn infer_pattern(function_ident: &syn::Ident) -> String {
    function_ident.to_string().replace('_', " ")
}

#[cfg(test)]
mod tests;
