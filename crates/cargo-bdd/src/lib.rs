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
pub fn unused(paths: &[PathBuf]) -> Result<Vec<&'static Step>> {
    let feature_steps = collect_feature_steps(paths)?;
    let mut by_kw: HashMap<StepKeyword, Vec<&'static Step>> = HashMap::new();
    for step in steps() {
        by_kw.entry(step.keyword).or_default().push(step);
    }

    let mut used: HashSet<*const Step> = HashSet::new();
    for (kw, text) in feature_steps {
        if let Some(candidates) = by_kw.get(&kw) {
            for step in candidates {
                if extract_placeholders(step.pattern, StepText::from(text.as_str())).is_ok() {
                    used.insert(*step as *const Step);
                }
            }
        }
    }

    Ok(steps()
        .into_iter()
        .filter(|s| !used.contains(&(*s as *const Step)))
        .collect())
}

fn collect_feature_steps(paths: &[PathBuf]) -> Result<Vec<(StepKeyword, String)>> {
    let mut files = HashSet::new();
    for path in paths {
        for file in gather_features(path)? {
            files.insert(file);
        }
    }

    let mut files: Vec<PathBuf> = files.into_iter().collect();
    files.sort();

    let mut out = Vec::new();
    for file in files {
        let feature = gherkin::Feature::parse_path(&file, gherkin::GherkinEnv::default())?;
        if let Some(bg) = &feature.background {
            out.extend(bg.steps.iter().map(map_step));
        }
        for scenario in &feature.scenarios {
            out.extend(scenario.steps.iter().map(map_step));
        }
    }
    Ok(out)
}

fn gather_features(path: &Path) -> Result<Vec<PathBuf>> {
    let meta = fs::metadata(path)?;
    if meta.is_file() {
        return if path.extension().is_some_and(|e| e == "feature") {
            Ok(vec![fs::canonicalize(path)?])
        } else {
            Ok(Vec::new())
        };
    }
    let mut out = Vec::new();
    for entry in WalkDir::new(path).into_iter().filter_map(Result::ok) {
        let p = entry.path();
        if entry.file_type().is_file() && p.extension().is_some_and(|e| e == OsStr::new("feature"))
        {
            out.push(fs::canonicalize(p)?);
        }
    }
    Ok(out)
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
