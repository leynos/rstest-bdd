//! Step registration, lookup, and placeholder matching.
//! Defines `Step`, the `step!` registration macro, and the global registry.

use std::sync::{LazyLock, Mutex};

use hashbrown::{HashMap, HashSet};
use inventory::iter;
use rstest_bdd_patterns::SpecificityScore;

use crate::{
    pattern::StepPattern,
    placeholder::extract_placeholders,
    types::{AsyncStepFn, PatternStr, StepExecutionMode, StepFn, StepKeyword, StepText},
};

mod async_lookup;
mod bypassed;
#[cfg(feature = "diagnostics")]
pub(crate) mod diagnostics;
/// Typed fixture requirement metadata for registered BDD steps.
mod fixtures;
mod introspection;
/// Stable library identities and closed scenario scopes.
mod library;
/// Step inventory registration macro implementation.
mod registration;

pub use async_lookup::{
    find_step_async_with_mode,
    find_step_with_mode,
    lookup_step_async_with_mode,
};
pub use bypassed::{BypassedScenario, record_bypassed_steps};
pub use fixtures::{FixtureRequirement, StepFixtureRequirements, fixture_requirements_for_step};
#[cfg(feature = "diagnostics")]
pub use introspection::dump_registry;
pub use introspection::{duplicate_steps, unused_steps};
pub use library::{GLOBAL_STEP_LIBRARY, StepLibrary, StepLibraryId, StepScope};

/// Represents a single step definition registered with the framework.
#[derive(Debug)]
pub struct Step {
    /// Rust module path of the definition, used to assign its library.
    pub module_path: &'static str,
    /// The step keyword, e.g. `Given` or `When`.
    pub keyword: StepKeyword,
    /// Pattern text used to match a Gherkin step.
    pub pattern: &'static StepPattern,
    /// Function pointer executed when the step is invoked (sync mode).
    pub run: StepFn,
    /// Function pointer executed when the step is invoked (async mode).
    ///
    /// For sync step definitions, this wraps the result in an immediately-ready
    /// future, enabling mixed sync and async steps within async scenarios.
    pub run_async: AsyncStepFn,
    /// Whether the step has a native sync body, a native async body, or both.
    pub execution_mode: StepExecutionMode,
    /// Names of fixtures this step requires.
    pub fixtures: &'static [&'static str],
    /// Source file where the step is defined.
    pub file: &'static str,
    /// Line number within the source file.
    pub line: u32,
}

inventory::collect!(Step);
inventory::collect!(StepLibrary);
inventory::collect!(StepFixtureRequirements);

/// Stable key used to identify a registered step.
type StepKey = (StepLibraryId, StepKeyword, &'static str);

/// Resolve module paths to declared library identities.
static LIBRARIES: LazyLock<Vec<StepLibrary>> =
    LazyLock::new(|| iter::<StepLibrary>.into_iter().copied().collect());

/// Assign a step to its nearest declared library, or to the global library.
pub(super) fn library_for_step(step: &Step) -> StepLibraryId {
    LIBRARIES
        .iter()
        .filter(|library| {
            step.module_path == library.id.as_str()
                || step
                    .module_path
                    .strip_prefix(library.id.as_str())
                    .is_some_and(|suffix| suffix.starts_with("::"))
        })
        .max_by_key(|library| library.id.as_str().len())
        .map_or(GLOBAL_STEP_LIBRARY, |library| library.id)
}

/// Lazily built map of registered steps by keyword and pattern.
static STEP_MAP: LazyLock<HashMap<StepKey, &'static Step>> = LazyLock::new(|| {
    let steps: Vec<_> = iter::<Step>.into_iter().collect();
    let mut map = HashMap::with_capacity(steps.len());
    for step in steps {
        if let Err(e) = step.pattern.compile() {
            panic!(
                "invalid step pattern '{}' at {}:{}: {e}",
                step.pattern.as_str(),
                step.file,
                step.line
            );
        }
        let key = (library_for_step(step), step.keyword, step.pattern.as_str());
        assert!(
            !map.contains_key(&key),
            "duplicate step for library '{}' + '{}' + '{}' defined at {}:{}",
            key.0.as_str(),
            step.keyword.as_str(),
            step.pattern.as_str(),
            step.file,
            step.line
        );
        map.insert(key, step);
    }
    map
});

/// Per-library index used by parameterized lookups.
static STEPS_BY_LIBRARY: LazyLock<HashMap<StepLibraryId, Vec<&'static Step>>> =
    LazyLock::new(|| {
        let mut index = HashMap::new();
        for step in iter::<Step> {
            index
                .entry(library_for_step(step))
                .or_insert_with(Vec::new)
                .push(step);
        }
        index
    });

// Tracks step invocations for the lifetime of the current process only. The
// data is not persisted across binaries, keeping usage bookkeeping lightweight
// and ephemeral.
/// Process-local set of steps observed during execution.
static USED_STEPS: LazyLock<Mutex<HashSet<StepKey>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

/// Mark a registered step as used.
fn mark_used(key: StepKey) {
    USED_STEPS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(key);
}

/// Record a resolved step as used by an execution boundary.
pub(crate) fn mark_step_used(step: &'static Step) {
    mark_used((library_for_step(step), step.keyword, step.pattern.as_str()));
}

/// Collect all steps submitted through `inventory`.
fn all_steps() -> Vec<&'static Step> { iter::<Step>.into_iter().collect() }

/// Look up a step by its stable registry key.
fn step_by_key(key: StepKey) -> Option<&'static Step> { STEP_MAP.get(&key).copied() }

/// Resolve a step whose registered pattern text exactly matches the input.
fn resolve_exact_step(
    scope: StepScope,
    keyword: StepKeyword,
    pattern: PatternStr<'_>,
) -> Result<Option<&'static Step>, StepLookupError> {
    let matches = scope
        .libraries()
        .iter()
        .filter_map(|library| {
            STEP_MAP
                .get(&(*library, keyword, pattern.as_str()))
                .copied()
        })
        .collect();
    select_most_specific(scope, keyword, pattern.as_str(), matches)
}

/// Resolve the most specific registered step matching the supplied text.
fn resolve_step(
    scope: StepScope,
    keyword: StepKeyword,
    text: StepText<'_>,
) -> Result<Option<&'static Step>, StepLookupError> {
    // Fast path: exact pattern match
    if let Some(step) = resolve_exact_step(scope, keyword, text.as_str().into())? {
        return Ok(Some(step));
    }

    // Find the most specific matching step directly via iterator
    let matches = scope
        .libraries()
        .iter()
        .flat_map(|library| STEPS_BY_LIBRARY.get(library).into_iter().flatten().copied())
        .filter(|step| step.keyword == keyword && extract_placeholders(step.pattern, text).is_ok())
        .collect();
    select_most_specific(scope, keyword, text.as_str(), matches)
}

/// Compute the specificity score for a step, logging any errors.
fn step_specificity(step: &Step) -> SpecificityScore {
    step.pattern.specificity().unwrap_or_else(|e| {
        log::warn!(
            "specificity calculation failed for pattern '{}': {e}",
            step.pattern.as_str()
        );
        SpecificityScore::default()
    })
}

/// Ambiguity found while resolving a scoped step.
#[derive(Clone, Debug)]
pub struct StepLookupError {
    /// Keyword used by the scenario step.
    pub keyword: StepKeyword,
    /// Scenario step text.
    pub text: String,
    /// Closed library list used for resolution.
    pub scope: StepScope,
    /// Equally specific matching definitions.
    pub candidates: Vec<&'static Step>,
}

impl std::fmt::Display for StepLookupError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let selected = self
            .scope
            .libraries()
            .iter()
            .map(|library| library.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let candidates = self
            .candidates
            .iter()
            .map(|step| {
                format!(
                    "{} '{}' at {}:{}",
                    library_for_step(step).as_str(),
                    step.pattern.as_str(),
                    step.file,
                    step.line
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        write!(
            formatter,
            "ambiguous {} '{}'; selected libraries: [{}]; candidates: {}",
            self.keyword.as_str(),
            self.text,
            selected,
            candidates
        )
    }
}

impl std::error::Error for StepLookupError {}

/// Select the unique most-specific candidate, or report the tied candidates.
fn select_most_specific(
    scope: StepScope,
    keyword: StepKeyword,
    text: &str,
    matches: Vec<&'static Step>,
) -> Result<Option<&'static Step>, StepLookupError> {
    let Some(top_specificity) = matches.iter().map(|step| step_specificity(step)).max() else {
        return Ok(None);
    };
    let candidates: Vec<_> = matches
        .into_iter()
        .filter(|step| step_specificity(step) == top_specificity)
        .collect();
    if candidates.len() == 1 {
        Ok(candidates.into_iter().next())
    } else {
        Err(StepLookupError {
            keyword,
            text: text.to_owned(),
            scope,
            candidates,
        })
    }
}

/// Mark a resolved step as used and apply a projection to it.
///
/// This is the canonical post-resolution path for lookup operations whose
/// contract records usage. The scoped metadata query remains side-effect free;
/// execution records its resolved step explicitly at the command boundary.
fn mark_and_project<T>(
    step: Option<&'static Step>,
    project: impl FnOnce(&'static Step) -> T,
) -> Option<T> {
    step.map(|found| {
        mark_step_used(found);
        project(found)
    })
}

/// Look up a registered step by keyword and pattern.
///
/// # Errors
///
/// Returns [`StepLookupError`] when equally specific definitions match.
pub fn lookup_step(
    keyword: StepKeyword,
    pattern: PatternStr<'_>,
) -> Result<Option<StepFn>, StepLookupError> {
    resolve_exact_step(StepScope::global(), keyword, pattern)
        .map(|step| mark_and_project(step, |found| found.run))
}

/// Find a registered step whose pattern matches the provided text.
///
/// # Errors
///
/// Returns [`StepLookupError`] when equally specific definitions match.
pub fn find_step(
    keyword: StepKeyword,
    text: StepText<'_>,
) -> Result<Option<StepFn>, StepLookupError> {
    resolve_step(StepScope::global(), keyword, text)
        .map(|step| mark_and_project(step, |found| found.run))
}

/// Look up a registered async step by keyword and pattern.
///
/// # Errors
///
/// Returns [`StepLookupError`] when equally specific definitions match.
pub fn lookup_step_async(
    keyword: StepKeyword,
    pattern: PatternStr<'_>,
) -> Result<Option<AsyncStepFn>, StepLookupError> {
    resolve_exact_step(StepScope::global(), keyword, pattern)
        .map(|step| mark_and_project(step, |found| found.run_async))
}

/// Find a registered async step whose pattern matches the provided text.
///
/// # Errors
///
/// Returns [`StepLookupError`] when equally specific definitions match.
pub fn find_step_async(
    keyword: StepKeyword,
    text: StepText<'_>,
) -> Result<Option<AsyncStepFn>, StepLookupError> {
    resolve_step(StepScope::global(), keyword, text)
        .map(|step| mark_and_project(step, |found| found.run_async))
}

/// Find a registered step and return its full metadata.
///
/// Unlike [`find_step`], this function returns the entire [`Step`] struct,
/// providing access to the step's required fixtures, source location, and
/// other metadata. This is useful for fixture validation and error reporting.
///
/// # Examples
///
/// ```ignore
/// use rstest_bdd::{find_step_with_metadata, StepKeyword, StepText};
///
/// if let Some(step) = find_step_with_metadata(StepKeyword::Given, StepText::from("a value"))? {
///     println!("Step requires fixtures: {:?}", step.fixtures);
///     // Invoke the step function
///     let result = (step.run)(&mut ctx, text, None, None);
/// }
/// # Ok::<(), rstest_bdd::StepLookupError>(())
/// ```
///
/// # Errors
///
/// Returns [`StepLookupError`] when equally specific definitions match.
pub fn find_step_with_metadata(
    keyword: StepKeyword,
    text: StepText<'_>,
) -> Result<Option<&'static Step>, StepLookupError> {
    resolve_step(StepScope::global(), keyword, text)
        .map(|step| mark_and_project(step, |found| found))
}

/// Find a registered step in `scope`, preserving ambiguity diagnostics.
///
/// This metadata query does not mark the resolved definition as used. Runtime
/// execution performs that mutation after resolution at its command boundary.
///
/// # Errors
///
/// Returns [`StepLookupError`] when multiple selected definitions are equally
/// specific matches for the step text.
pub fn find_step_with_metadata_in_scope(
    scope: StepScope,
    keyword: StepKeyword,
    text: StepText<'_>,
) -> Result<Option<&'static Step>, StepLookupError> {
    resolve_step(scope, keyword, text)
}
