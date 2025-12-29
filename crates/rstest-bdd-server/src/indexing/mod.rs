//! Indexing pipelines used by the language server.
//!
//! Phase 7 focuses on building a reliable indexing foundation. The first step
//! is parsing saved sources and capturing:
//!
//! - Feature steps (keyword, text, step span)
//! - Feature doc strings, data tables, and Examples header columns
//! - Rust step definitions annotated with `#[given]`, `#[when]`, and `#[then]`
//!   (keyword, pattern string, parameters, table/doc string expectations)
//!
//! The implementation relies on the `gherkin` crate for syntactic parsing.
//! Where `gherkin` does not expose spans (for example doc string blocks and
//! per-cell column offsets), the indexer performs a lightweight scan of the
//! raw feature text to derive stable byte offsets.

use std::path::PathBuf;

use gherkin::{Span, StepType};

mod feature;
mod registry;
mod rust;

pub use feature::{index_feature_file, index_feature_source};
pub use registry::{CompiledStepDefinition, StepDefinitionRegistry, StepPatternCompileError};
pub use rust::{index_rust_file, index_rust_source};

/// Parsed index for a single `.feature` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeatureFileIndex {
    /// Source path for the indexed feature.
    pub path: PathBuf,
    /// The normalized source text of the feature file.
    ///
    /// Stored alongside the index to avoid re-reading from disk on navigation
    /// requests. The source is normalized to always end with a newline, matching
    /// how the gherkin parser processes the input.
    pub source: String,
    /// All steps found in the feature (including backgrounds and rules).
    pub steps: Vec<IndexedStep>,
    /// Example header columns extracted from scenario outlines.
    pub example_columns: Vec<IndexedExampleColumn>,
}

/// A step captured from a Gherkin feature file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedStep {
    /// The raw step keyword used in the source (includes `And` and `But`).
    pub keyword: String,
    /// The contextual step type resolved by the parser (Given/When/Then).
    pub step_type: StepType,
    /// The step text following the keyword.
    pub text: String,
    /// Byte span for the step line in the source.
    pub span: Span,
    /// Attached doc string content and its byte span, if present.
    pub docstring: Option<IndexedDocstring>,
    /// Attached data table rows and its byte span, if present.
    pub table: Option<IndexedTable>,
}

/// A doc string attached to a step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedDocstring {
    /// Doc string content (as parsed by `gherkin`).
    pub value: String,
    /// Byte span covering the doc string block in the source.
    pub span: Span,
}

/// A data table attached to a step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedTable {
    /// Table rows, as parsed by `gherkin`.
    pub rows: Vec<Vec<String>>,
    /// Byte span covering the table block in the source.
    pub span: Span,
}

/// A scenario outline Examples header column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedExampleColumn {
    /// The column name (header cell contents).
    pub name: String,
    /// Byte span covering the header cell contents in the source.
    pub span: Span,
}

/// Errors that can occur during `.feature` indexing.
#[derive(Debug, thiserror::Error)]
pub enum FeatureIndexError {
    /// Failed to read the source `.feature` file.
    #[error("failed to read feature file: {0}")]
    Read(#[from] std::io::Error),
    /// Failed to parse the `.feature` file with the Gherkin parser.
    #[error("failed to parse feature file: {0}")]
    Parse(#[from] gherkin::ParseError),
    /// The feature file contained a doc string, but no delimiter block could
    /// be located in the source text.
    #[error("failed to locate doc string span for step at {0:?}")]
    DocstringSpanNotFound(Span),
}

/// Parsed index for a single Rust source file containing step definitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustStepFileIndex {
    /// Source path for the indexed Rust file.
    pub path: PathBuf,
    /// Step definitions collected from the file.
    pub step_definitions: Vec<IndexedStepDefinition>,
}

/// A Rust function annotated with `#[given]`, `#[when]`, or `#[then]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedStepDefinition {
    /// The step keyword provided by the macro attribute.
    pub keyword: StepType,
    /// The step pattern string registered by the macro.
    pub pattern: String,
    /// Whether the pattern was inferred from the function name.
    pub pattern_inferred: bool,
    /// The Rust function that provides the step implementation.
    pub function: RustFunctionId,
    /// The function's parameters, in source order.
    pub parameters: Vec<IndexedStepParameter>,
    /// Whether the step expects a data table argument.
    pub expects_table: bool,
    /// Whether the step expects a doc string argument.
    pub expects_docstring: bool,
    /// 0-based line number of the function definition in the Rust source.
    pub line: u32,
}

/// Stable identifier for a Rust function within a source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustFunctionId {
    /// Modules containing the function, in declaration order.
    pub module_path: Vec<String>,
    /// The function name.
    pub name: String,
}

/// A parameter declared on a step definition function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedStepParameter {
    /// The parameter name, when it can be represented as an identifier.
    pub name: Option<String>,
    /// Token-string representation of the parameter type.
    pub ty: String,
    /// Whether the parameter is treated as the step's data table argument.
    pub is_datatable: bool,
    /// Whether the parameter is treated as the step's doc string argument.
    pub is_docstring: bool,
}

/// Errors that can occur during Rust step definition indexing.
#[derive(Debug, thiserror::Error)]
pub enum RustStepIndexError {
    /// Failed to read the Rust source file.
    #[error("failed to read rust source file: {0}")]
    Read(#[from] std::io::Error),
    /// Failed to parse the Rust source into a syntax tree.
    #[error("failed to parse rust source: {0}")]
    Parse(#[from] syn::Error),
    /// A step function was annotated with multiple step attributes.
    #[error("step function '{function}' has multiple step attributes")]
    MultipleStepAttributes {
        /// Function name used for the diagnostic.
        function: String,
    },
    /// Failed to interpret the step attribute arguments.
    #[error("invalid arguments for #[{attribute}] on step function '{function}': {message}")]
    InvalidStepAttributeArguments {
        /// Function name used for the diagnostic.
        function: String,
        /// Attribute keyword (`given`, `when`, or `then`).
        attribute: &'static str,
        /// Human-readable parse error message.
        message: String,
    },
}
