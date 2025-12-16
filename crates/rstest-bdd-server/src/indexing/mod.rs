//! Indexing pipelines used by the language server.
//!
//! Phase 7 focuses on building a reliable indexing foundation. The first step
//! is parsing `.feature` files on save and capturing:
//!
//! - Steps (keyword, text, step span)
//! - Attached doc strings and data tables
//! - Scenario outline Examples header columns
//! - Byte offsets for the indexed elements
//!
//! The implementation relies on the `gherkin` crate for syntactic parsing.
//! Where `gherkin` does not expose spans (for example doc string blocks and
//! per-cell column offsets), the indexer performs a lightweight scan of the
//! raw feature text to derive stable byte offsets.

use std::path::PathBuf;

use gherkin::{Span, StepType};

mod feature;

pub use feature::{index_feature_file, index_feature_source};

/// Parsed index for a single `.feature` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeatureFileIndex {
    /// Source path for the indexed feature.
    pub path: PathBuf,
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
