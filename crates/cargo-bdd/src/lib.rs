//! Diagnostic helpers for inspecting `rstest-bdd` step definitions.
//!
//! This library powers the `cargo-bdd` command and exposes utilities
//! to enumerate registered steps, detect duplicates, and flag
//! definitions that are not referenced by any provided feature files.

use eyre::Result;
use once_cell::sync::Lazy;
use regex::Regex;
use rstest_bdd::{Step, StepKeyword, StepText, extract_placeholders};
use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Gather all registered steps sorted by source location.
#[must_use]
pub fn steps() -> Vec<&'static Step> {
    let mut out: Vec<&'static Step> = inventory::iter::<Step>.into_iter().collect();
    out.sort_by_key(|s| (s.file, s.line));
    out
}

/// Group step definitions that share the same keyword and pattern.
#[must_use]
pub fn duplicates() -> Vec<Vec<&'static Step>> {
    let mut map: HashMap<(StepKeyword, String), Vec<&'static Step>> = HashMap::new();
    for step in steps() {
        let key = (step.keyword, normalise_pattern(step.pattern.as_str()));
        map.entry(key).or_default().push(step);
    }
    map.into_values().filter(|v| v.len() > 1).collect()
}

static PLACEHOLDER_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"<[^>]+>").expect("valid placeholder regex"));

fn normalise_pattern(pat: &str) -> String {
    let collapsed = pat.split_whitespace().collect::<Vec<_>>().join(" ");
    PLACEHOLDER_RE.replace_all(&collapsed, "<>").into_owned()
}

/// Identify step definitions that are unused in the given feature paths.
///
/// `paths` may point to either `.feature` files or directories containing them.
///
/// # Errors
/// Returns an error if any path cannot be read or parsed.
///
/// ```
/// # use std::path::PathBuf;
/// # fn demo() -> eyre::Result<()> {
/// let unused = unused(&[PathBuf::from("tests/features")])?;
/// assert!(!unused.is_empty());
/// # Ok(())
/// # }
/// ```
pub fn unused(paths: &[PathBuf]) -> Result<Vec<&'static Step>> {
    let feature_steps = collect_feature_steps(paths)?;
    let used = find_used_steps(feature_steps);

    Ok(steps()
        .into_iter()
        .filter(|s| !used.contains(&(*s as *const Step)))
        .collect())
}

/// Determine which registered steps are referenced by the feature steps.
///
/// ```
/// use rstest_bdd::StepKeyword;
/// let used = find_used_steps(vec![(StepKeyword::Given, "step".into())]);
/// assert!(used.is_empty());
/// ```
fn find_used_steps(feature_steps: Vec<(StepKeyword, String)>) -> HashSet<*const Step> {
    let by_kw = group_steps_by_keyword();
    let mut used = HashSet::new();
    for (kw, text) in feature_steps {
        if let Some(candidates) = by_kw.get(&kw) {
            find_matching_steps(candidates, &text, &mut used);
        }
    }
    used
}

/// Group registered steps by keyword for faster lookup.
///
/// ```
/// let map = group_steps_by_keyword();
/// assert!(map.keys().next().is_some());
/// ```
fn group_steps_by_keyword() -> HashMap<StepKeyword, Vec<&'static Step>> {
    let mut map: HashMap<StepKeyword, Vec<&'static Step>> = HashMap::new();
    for step in steps() {
        map.entry(step.keyword).or_default().push(step);
    }
    map
}

/// Record steps that match the provided text.
///
/// ```
/// # use rstest_bdd::Step;
/// # use std::collections::HashSet;
/// # fn demo(step: &Step, text: &str) {
/// let mut used = HashSet::new();
/// find_matching_steps(&[step], text, &mut used);
/// # }
/// ```
fn find_matching_steps(candidates: &[&'static Step], text: &str, used: &mut HashSet<*const Step>) {
    for step in candidates {
        if matches_step_pattern(step, text) {
            used.insert(*step as *const Step);
        }
    }
}

/// Check if a step pattern matches the provided text.
///
/// ```
/// # use rstest_bdd::Step;
/// # fn demo(step: &Step, text: &str) {
/// let _ = matches_step_pattern(step, text);
/// # }
/// ```
fn matches_step_pattern(step: &Step, text: &str) -> bool {
    extract_placeholders(step.pattern, StepText::from(text)).is_ok()
}

/// Gather steps referenced in the supplied feature paths.
///
/// ```
/// # use std::path::PathBuf;
/// # fn demo() -> eyre::Result<()> {
/// let steps = collect_feature_steps(&[PathBuf::from("tests/features")])?;
/// assert!(!steps.is_empty());
/// # Ok(())
/// # }
/// ```
fn collect_feature_steps(paths: &[PathBuf]) -> Result<Vec<(StepKeyword, String)>> {
    let files = collect_unique_feature_files(paths)?;
    let mut out = Vec::new();
    for file in files {
        out.extend(extract_steps_from_feature(&file)?);
    }
    Ok(out)
}

/// Collect unique, sorted feature files from the provided paths.
///
/// ```
/// # use std::path::PathBuf;
/// # fn demo() -> eyre::Result<()> {
/// let files = collect_unique_feature_files(&[PathBuf::from("tests/features")])?;
/// assert!(!files.is_empty());
/// # Ok(())
/// # }
/// ```
fn collect_unique_feature_files(paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut files = HashSet::new();
    for path in paths {
        for file in gather_features(path)? {
            files.insert(file);
        }
    }
    let mut files: Vec<PathBuf> = files.into_iter().collect();
    files.sort();
    Ok(files)
}

/// Parse a single feature file and extract its steps.
///
/// ```
/// # use std::path::Path;
/// # fn demo(path: &Path) -> eyre::Result<()> {
/// let steps = extract_steps_from_feature(path)?;
/// assert!(!steps.is_empty());
/// # Ok(())
/// # }
/// ```
fn extract_steps_from_feature(file: &Path) -> Result<Vec<(StepKeyword, String)>> {
    let feature = gherkin::Feature::parse_path(file, gherkin::GherkinEnv::default())?;
    let mut out = Vec::new();
    if let Some(bg) = &feature.background {
        out.extend(bg.steps.iter().map(map_step));
    }
    for scenario in &feature.scenarios {
        out.extend(scenario.steps.iter().map(map_step));
    }
    Ok(out)
}

/// Recursively collect `.feature` files from the given path.
///
/// ```
/// # use std::path::Path;
/// # fn demo(path: &Path) -> eyre::Result<()> {
/// let files = gather_features(path)?;
/// assert!(files.iter().all(|p| p.extension().is_some()));
/// # Ok(())
/// # }
/// ```
fn gather_features(path: &Path) -> Result<Vec<PathBuf>> {
    let meta = fs::metadata(path)?;
    if meta.is_file() {
        return if is_feature_file(path) {
            Ok(vec![fs::canonicalize(path)?])
        } else {
            Ok(Vec::new())
        };
    }

    let files = WalkDir::new(path)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path().to_path_buf())
        .filter(|p| is_feature_file(p))
        .map(fs::canonicalize)
        .collect::<std::io::Result<Vec<_>>>()?;
    Ok(files)
}

/// Check whether the supplied path has a `.feature` extension.
///
/// ```
/// # use std::path::Path;
/// assert!(is_feature_file(Path::new("foo.feature")));
/// ```
fn is_feature_file(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == OsStr::new("feature"))
}

fn map_step(step: &gherkin::Step) -> (StepKeyword, String) {
    let kw = match step.keyword.as_str() {
        "And" => StepKeyword::And,
        "But" => StepKeyword::But,
        _ => step.ty.into(),
    };
    (kw, step.value.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn detects_duplicates() {
        assert!(!duplicates().is_empty());
    }

    #[test]
    fn finds_unused_steps() {
        let unused = unused(&[PathBuf::from("tests/features")]).unwrap();
        assert!(unused.iter().any(|s| s.pattern.as_str() == "unused step"));
    }
}

#[cfg(feature = "test-steps")]
mod test_steps;
