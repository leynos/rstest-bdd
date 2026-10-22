//! Resolving a scenario step to its single most specific definition.
//!
//! Translation happens in two stages, and the split between them is what this
//! module exists to keep visible. *Matching* asks which registered definitions
//! could accept the text: an exact pattern lookup, then a parameterized scan
//! over the selected libraries. *Selection* asks which of those matches to run,
//! and refuses when two are equally specific rather than guessing.
//!
//! The refusal is the point. `#[step_library]` makes it normal for several
//! libraries to define the same phrase, so a tie means the scenario's scope was
//! under-specified, and picking a winner by declaration order or first match
//! would silently run a step from a library the scenario never named. Instead
//! the tie is reported as [`StepLookupError`] carrying the scope and every
//! candidate's source location, so the diagnostic names the definitions the
//! author has to choose between.
//!
//! Matching and selection are also the two halves of the usage-marking
//! invariant. Resolution here is pure: it reads the registry and returns a
//! definition without recording anything. Recording happens in
//! [`super::mark_and_project`], which the public lookup wrappers call on the way
//! out. Keeping resolution side-effect free is what lets the language server
//! and preflight checks resolve steps without making them look executed, while
//! `cargo bdd unused` still reflects the steps that actually ran.

use hashbrown::HashSet;
use rstest_bdd_patterns::SpecificityScore;

use super::{STEP_MAP, STEPS_BY_LIBRARY, Step, library::StepScope, library_for_step};
use crate::{
    placeholder::extract_placeholders,
    types::{PatternStr, StepKeyword, StepText},
};

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

/// Resolve a step whose registered pattern text exactly matches the input.
pub(super) fn resolve_exact_step(
    scope: StepScope,
    keyword: StepKeyword,
    pattern: PatternStr<'_>,
) -> Result<Option<&'static Step>, StepLookupError> {
    let mut seen_libraries = HashSet::new();
    let matches = scope
        .libraries()
        .iter()
        .filter(|library| seen_libraries.insert(**library))
        .filter_map(|library| {
            STEP_MAP
                .get(&(*library, keyword, pattern.as_str()))
                .copied()
        })
        .collect();
    select_most_specific(scope, keyword, pattern.as_str(), matches)
}

/// Resolve the most specific registered step matching the supplied text.
pub(super) fn resolve_step(
    scope: StepScope,
    keyword: StepKeyword,
    text: StepText<'_>,
) -> Result<Option<&'static Step>, StepLookupError> {
    // Fast path: exact pattern match
    if let Some(step) = resolve_exact_step(scope, keyword, text.as_str().into())? {
        return Ok(Some(step));
    }

    // Find the most specific matching step directly via iterator
    let mut seen_libraries = HashSet::new();
    let matches = scope
        .libraries()
        .iter()
        .filter(|library| seen_libraries.insert(**library))
        .flat_map(|library| STEPS_BY_LIBRARY.get(library).into_iter().flatten().copied())
        .filter(|step| step.keyword == keyword && extract_placeholders(step.pattern, text).is_ok())
        .collect();
    select_most_specific(scope, keyword, text.as_str(), matches)
}

/// Compute the specificity score for a step, logging any errors.
///
/// A pattern that cannot be scored is treated as least specific rather than as
/// a hard failure: the pattern was already compiled by `STEP_MAP`, so a scoring
/// error here is a defect in one definition, and degrading that definition's
/// rank keeps every other scenario in the binary runnable.
fn step_specificity(step: &Step) -> SpecificityScore {
    step.pattern.specificity().unwrap_or_else(|e| {
        tracing::warn!(
            "specificity calculation failed for pattern '{}': {e}",
            step.pattern.as_str()
        );
        SpecificityScore::default()
    })
}

/// Select the unique most-specific candidate, or report the tied candidates.
///
/// Ties among *equally specific* definitions are the error; a strict maximum is
/// a unique winner and resolves. When nothing matched, `Ok(None)` distinguishes
/// "no definition matches" from "several match equally well", which callers
/// surface differently.
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
