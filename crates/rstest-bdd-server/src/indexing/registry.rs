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

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use gherkin::StepType;
use regex::Regex;
use rstest_bdd_patterns::{PatternError, compile_regex_from_pattern};

use super::{IndexedStepDefinition, RustFunctionId, RustStepFileIndex};

/// A Rust step definition with a compiled regular expression.
#[derive(Debug, Clone)]
pub struct CompiledStepDefinition {
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
    /// Whether the step expects a data table argument.
    pub expects_table: bool,
    /// Whether the step expects a doc string argument.
    pub expects_docstring: bool,
}

/// Error raised when a step pattern cannot be compiled.
#[derive(Debug, thiserror::Error)]
#[error(
    "failed to compile step pattern '{pattern}' for {keyword:?} step '{function}' in {path}: {source}"
)]
pub struct StepPatternCompileError {
    /// Absolute path to the Rust source file containing the step.
    pub path: String,
    /// Fully qualified function name (module path + function identifier).
    pub function: String,
    /// Step keyword (Given/When/Then).
    pub keyword: StepType,
    /// The original pattern string.
    pub pattern: String,
    /// The underlying pattern compilation error.
    #[source]
    pub source: PatternError,
}

impl StepPatternCompileError {
    fn new(path: &Path, step: &IndexedStepDefinition, source: PatternError) -> Self {
        Self {
            path: path.display().to_string(),
            function: format_function_id(&step.function),
            keyword: step.keyword,
            pattern: step.pattern.clone(),
            source,
        }
    }
}

fn format_function_id(function: &RustFunctionId) -> String {
    if function.module_path.is_empty() {
        return function.name.clone();
    }

    let mut fq = String::new();
    for segment in &function.module_path {
        fq.push_str(segment);
        fq.push_str("::");
    }
    fq.push_str(&function.name);
    fq
}

/// In-memory registry of compiled step patterns.
#[derive(Debug, Default)]
pub struct StepDefinitionRegistry {
    steps_by_file: HashMap<PathBuf, Vec<CompiledStepDefinition>>,
    steps_by_keyword: HashMap<StepType, Vec<CompiledStepDefinition>>,
}

impl StepDefinitionRegistry {
    /// Replace all compiled step definitions for a single Rust source file.
    ///
    /// This method invalidates previously compiled entries for the same path
    /// and then repopulates the registry from the provided file index.
    pub fn replace_rust_file(&mut self, index: &RustStepFileIndex) -> Vec<StepPatternCompileError> {
        self.invalidate_file(&index.path);

        let mut compiled = Vec::new();
        let mut errors = Vec::new();
        for step in &index.step_definitions {
            match compile_step_definition(&index.path, step) {
                Ok(step) => compiled.push(step),
                Err(err) => errors.push(*err),
            }
        }

        if !compiled.is_empty() {
            self.steps_by_file
                .insert(index.path.clone(), compiled.clone());
            for step in compiled {
                self.steps_by_keyword
                    .entry(step.keyword)
                    .or_default()
                    .push(step);
            }
        }

        errors
    }

    /// Remove all compiled step definitions for a given Rust source path.
    pub fn invalidate_file(&mut self, path: &Path) {
        self.steps_by_file.remove(path);
        for steps in self.steps_by_keyword.values_mut() {
            steps.retain(|step| step.source_path.as_path() != path);
        }
    }

    /// Return compiled steps for a given keyword.
    #[must_use]
    pub fn steps_for_keyword(&self, keyword: StepType) -> &[CompiledStepDefinition] {
        self.steps_by_keyword
            .get(&keyword)
            .map_or(&[], Vec::as_slice)
    }

    /// Return compiled steps originating from a single Rust source file.
    #[must_use]
    pub fn steps_for_file(&self, path: &Path) -> &[CompiledStepDefinition] {
        self.steps_by_file
            .get(path)
            .map_or(&[], Vec::as_slice)
    }
}

fn compile_step_definition(
    path: &Path,
    step: &IndexedStepDefinition,
) -> Result<CompiledStepDefinition, Box<StepPatternCompileError>> {
    let regex = compile_regex_from_pattern(&step.pattern)
        .map_err(|err| Box::new(StepPatternCompileError::new(path, step, err)))?;

    Ok(CompiledStepDefinition {
        keyword: step.keyword,
        pattern: step.pattern.clone(),
        pattern_inferred: step.pattern_inferred,
        regex,
        function: step.function.clone(),
        source_path: path.to_path_buf(),
        expects_table: step.expects_table,
        expects_docstring: step.expects_docstring,
    })
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "tests use explicit failures for clarity"
)]
mod tests {
    use super::*;
    use crate::indexing::index_rust_source;

    #[test]
    fn replaces_file_entries_and_populates_keyword_registry() {
        let source = concat!(
            "use rstest_bdd_macros::{given, when};\n",
            "\n",
            "#[given(\"I have {n:u32}\")]\n",
            "fn have_number() {}\n",
            "\n",
            "#[when(\"I add 1\")]\n",
            "fn add_one() {}\n",
        );

        let index =
            index_rust_source(PathBuf::from("/tmp/steps.rs"), source).expect("index rust source");

        let mut registry = StepDefinitionRegistry::default();
        let errors = registry.replace_rust_file(&index);
        assert!(
            errors.is_empty(),
            "expected patterns to compile successfully: {errors:?}"
        );

        let given = registry.steps_for_keyword(StepType::Given);
        assert_eq!(given.len(), 1);
        let matcher = given.first().expect("compiled given matcher");
        assert!(matcher.regex.is_match("I have 42"));

        let when = registry.steps_for_keyword(StepType::When);
        assert_eq!(when.len(), 1);
        let matcher = when.first().expect("compiled when matcher");
        assert!(matcher.regex.is_match("I add 1"));
    }

    #[test]
    fn invalidates_entries_for_a_single_file_incrementally() {
        let path = PathBuf::from("/tmp/steps.rs");
        let first = concat!(
            "use rstest_bdd_macros::{given, when};\n",
            "\n",
            "#[given(\"a\")]\n",
            "fn step_a() {}\n",
            "\n",
            "#[when(\"b\")]\n",
            "fn step_b() {}\n",
        );
        let second = concat!(
            "use rstest_bdd_macros::given;\n",
            "\n",
            "#[given(\"a\")]\n",
            "fn step_a() {}\n",
        );

        let index_first = index_rust_source(path.clone(), first).expect("index first source");
        let index_second = index_rust_source(path.clone(), second).expect("index second source");

        let mut registry = StepDefinitionRegistry::default();
        registry.replace_rust_file(&index_first);
        assert_eq!(registry.steps_for_keyword(StepType::Given).len(), 1);
        assert_eq!(registry.steps_for_keyword(StepType::When).len(), 1);

        registry.replace_rust_file(&index_second);
        assert_eq!(registry.steps_for_keyword(StepType::Given).len(), 1);
        assert_eq!(registry.steps_for_keyword(StepType::When).len(), 0);
        assert_eq!(registry.steps_for_file(&path).len(), 1);
    }
}
