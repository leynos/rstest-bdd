//! Compiled Rust step-definition registry.
//!
//! The language server indexes Rust files on save and extracts step
//! definitions annotated with `#[given]`, `#[when]`, and `#[then]`. This module
//! compiles those patterns with `rstest-bdd-patterns` and stores the resulting
//! regular expressions in an in-memory registry keyed by the step keyword
//! (`Given`, `When`, or `Then`).
//!
//! The registry is updated incrementally: updating one Rust file removes the
//! previously compiled entries for that file and replaces them with the newly
//! indexed steps. This avoids rebuilding state for the entire workspace on
//! every save while ensuring stale entries are not retained.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use gherkin::StepType;
use regex::Regex;
use rstest_bdd_patterns::{PatternError, compile_regex_from_pattern};

use super::{
    IndexedStepDefinition,
    IndexedStepParameter,
    RustAttributeSpan,
    RustFunctionId,
    RustStepFileIndex,
};

/// A Rust step definition with a compiled regular expression.
#[derive(Debug, Clone)]
pub struct CompiledStepDefinition {
    /// Nearest lexical step library, or the built-in global library.
    pub library: String,
    /// The step keyword (Given/When/Then) selected by the macro attribute.
    pub keyword: StepType,
    /// The original pattern string registered by the macro.
    pub pattern: String,
    /// Whether the pattern was inferred from the function name.
    pub pattern_inferred: bool,
    /// The compiled regular expression for matching feature step text.
    pub regex: Regex,
    /// The Rust function that implements the step.
    pub function: RustFunctionId,
    /// Absolute path to the Rust source file containing the step.
    pub source_path: PathBuf,
    /// The function's parameters, in source order.
    pub parameters: Vec<IndexedStepParameter>,
    /// Whether the step expects a data table argument.
    pub expects_table: bool,
    /// Whether the step expects a doc string argument.
    pub expects_docstring: bool,
    /// Span of the step attribute (e.g., `#[given("...")]`) in the Rust source.
    pub attribute_span: RustAttributeSpan,
}

/// Error raised when a step pattern cannot be compiled.
#[derive(Debug, thiserror::Error)]
#[error(
    "failed to compile step pattern '{pattern}' for {keyword:?} step '{function}' in {path}: \
     {source}"
)]
pub struct StepPatternCompileError {
    /// Absolute path to the Rust source file containing the step.
    pub path: Box<str>,
    /// Fully qualified function name (module path + function identifier).
    pub function: Box<str>,
    /// Step keyword (Given/When/Then).
    pub keyword: StepType,
    /// The original pattern string.
    pub pattern: Box<str>,
    /// The underlying pattern compilation error.
    #[source]
    pub source: PatternError,
}

impl StepPatternCompileError {
    /// Build a compilation error with context from an indexed step.
    fn new(path: &Path, step: &IndexedStepDefinition, source: PatternError) -> Self {
        Self {
            path: path.display().to_string().into_boxed_str(),
            function: format_function_id(&step.function).into_boxed_str(),
            keyword: step.keyword,
            pattern: step.pattern.clone().into_boxed_str(),
            source,
        }
    }
}

/// Render a function identifier with its module path.
fn format_function_id(function: &RustFunctionId) -> String {
    if function.module_path.is_empty() {
        return function.name.clone();
    }

    format!("{}::{}", function.module_path.join("::"), function.name)
}

/// In-memory registry of compiled step patterns.
#[derive(Debug, Default)]
pub struct StepDefinitionRegistry {
    /// Compiled steps grouped by their source file.
    steps_by_file: HashMap<PathBuf, Vec<Arc<CompiledStepDefinition>>>,
    /// Compiled steps grouped by Gherkin keyword.
    steps_by_keyword: HashMap<StepType, Vec<Arc<CompiledStepDefinition>>>,
    /// Reverse mapping from source files to keyword entries.
    reverse_index: HashMap<PathBuf, Vec<ReverseIndexEntry>>,
    /// Positions of compiled steps within each keyword vector.
    keyword_positions: HashMap<StepType, HashMap<usize, usize>>,
}

/// Identifies one compiled step within a keyword index.
#[derive(Debug, Clone, Copy)]
struct ReverseIndexEntry {
    /// Gherkin keyword containing the entry.
    keyword: StepType,
    /// Stable pointer key for the compiled step.
    key: usize,
}

impl StepDefinitionRegistry {
    /// Replace all compiled step definitions for a single Rust source file.
    ///
    /// This method invalidates previously compiled entries for the same path
    /// and then repopulates the registry from the provided file index.
    pub fn replace_rust_file(&mut self, index: &RustStepFileIndex) -> Vec<StepPatternCompileError> {
        self.invalidate_file(&index.path);

        let (compiled, errors) = Self::compile_steps(index);
        self.insert_compiled_steps(&index.path, compiled);
        errors
    }

    /// Compile each indexed step definition for one Rust source file.
    fn compile_steps(
        index: &RustStepFileIndex,
    ) -> (Vec<CompiledStepDefinition>, Vec<StepPatternCompileError>) {
        let mut compiled = Vec::new();
        let mut errors = Vec::new();

        for step in &index.step_definitions {
            match compile_step_definition(&index.path, step) {
                Ok(step) => compiled.push(step),
                Err(err) => errors.push(err),
            }
        }

        (compiled, errors)
    }

    /// Insert compiled steps into the per-file and per-keyword indexes.
    fn insert_compiled_steps(&mut self, path: &Path, compiled: Vec<CompiledStepDefinition>) {
        if compiled.is_empty() {
            return;
        }

        let shared: Vec<_> = compiled.into_iter().map(Arc::new).collect();

        let mut reverse_entries = Vec::with_capacity(shared.len());

        for step in &shared {
            let key = Arc::as_ptr(step) as usize;

            let steps = self.steps_by_keyword.entry(step.keyword).or_default();
            let index = steps.len();
            steps.push(Arc::clone(step));
            self.keyword_positions
                .entry(step.keyword)
                .or_default()
                .insert(key, index);

            reverse_entries.push(ReverseIndexEntry {
                keyword: step.keyword,
                key,
            });
        }

        self.reverse_index
            .insert(path.to_path_buf(), reverse_entries);
        self.steps_by_file.insert(path.to_path_buf(), shared);
    }

    /// Remove all compiled step definitions for a given Rust source path.
    pub fn invalidate_file(&mut self, path: &Path) {
        self.steps_by_file.remove(path);
        let Some(entries) = self.reverse_index.remove(path) else {
            return;
        };

        for entry in entries {
            self.remove_keyword_entry(entry);
        }
    }

    /// Remove one compiled step from its keyword index.
    fn remove_keyword_entry(&mut self, ReverseIndexEntry { keyword, key }: ReverseIndexEntry) {
        let Some(steps) = self.steps_by_keyword.get_mut(&keyword) else {
            return;
        };
        let Some(positions) = self.keyword_positions.get_mut(&keyword) else {
            return;
        };
        let Some(&index) = positions.get(&key) else {
            return;
        };

        let _removed = steps.swap_remove(index);
        positions.remove(&key);

        if let Some(moved) = steps.get(index) {
            let moved_key = Arc::as_ptr(moved) as usize;
            positions.insert(moved_key, index);
        }

        if steps.is_empty() {
            self.steps_by_keyword.remove(&keyword);
            self.keyword_positions.remove(&keyword);
        }
    }

    /// Return compiled steps for a given keyword.
    #[must_use]
    pub fn steps_for_keyword(&self, keyword: StepType) -> &[Arc<CompiledStepDefinition>] {
        self.steps_by_keyword
            .get(&keyword)
            .map_or(&[], Vec::as_slice)
    }

    /// Return keyword-compatible definitions from the closed selected libraries.
    #[must_use]
    pub fn steps_for_keyword_in_scope(
        &self,
        keyword: StepType,
        libraries: &[String],
    ) -> Vec<&Arc<CompiledStepDefinition>> {
        self.steps_for_keyword(keyword)
            .iter()
            .filter(|step| libraries.iter().any(|library| library == &step.library))
            .collect()
    }

    /// Return compiled steps originating from a single Rust source file.
    #[must_use]
    pub fn steps_for_file(&self, path: &Path) -> &[Arc<CompiledStepDefinition>] {
        self.steps_by_file.get(path).map_or(&[], Vec::as_slice)
    }
}

/// Compile one indexed step definition into a registry entry.
fn compile_step_definition(
    path: &Path,
    step: &IndexedStepDefinition,
) -> Result<CompiledStepDefinition, StepPatternCompileError> {
    let regex = compile_regex_from_pattern(&step.pattern)
        .map_err(|err| StepPatternCompileError::new(path, step, err))?;

    Ok(CompiledStepDefinition {
        library: step.library.clone(),
        keyword: step.keyword,
        pattern: step.pattern.clone(),
        pattern_inferred: step.pattern_inferred,
        regex,
        function: step.function.clone(),
        source_path: path.to_path_buf(),
        parameters: step.parameters.clone(),
        expects_table: step.expects_table,
        expects_docstring: step.expects_docstring,
        attribute_span: step.attribute_span,
    })
}

#[cfg(test)]
mod tests;
