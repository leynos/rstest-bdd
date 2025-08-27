//! Diagnostic helpers for inspecting `rstest-bdd` step definitions.
//!
//! This library powers the `cargo-bdd` command and exposes utilities
//! to enumerate registered steps, detect duplicates, and flag
//! definitions that are not referenced by any provided feature files.

use eyre::Result;
use rstest_bdd::{Step, StepKeyword, StepText, extract_placeholders};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};

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
    let mut map: HashMap<(StepKeyword, &str), Vec<&'static Step>> = HashMap::new();
    for step in steps() {
        map.entry((step.keyword, step.pattern.as_str()))
            .or_default()
            .push(step);
    }
    map.into_values().filter(|v| v.len() > 1).collect()
}

/// Identify step definitions that are unused in the given feature paths.
///
/// `paths` may point to either `.feature` files or directories containing them.
///
/// # Errors
/// Returns an error if any path cannot be read or parsed.
pub fn unused(paths: &[PathBuf]) -> Result<Vec<&'static Step>> {
    let feature_steps = collect_feature_steps(paths)?;
    let mut used: HashSet<*const Step> = HashSet::new();
    for (kw, text) in feature_steps {
        for step in steps() {
            if step.keyword == kw
                && extract_placeholders(step.pattern, StepText::from(text.as_str())).is_ok()
            {
                used.insert(step as *const Step);
            }
        }
    }
    Ok(steps()
        .into_iter()
        .filter(|s| !used.contains(&(*s as *const Step)))
        .collect())
}

fn collect_feature_steps(paths: &[PathBuf]) -> Result<Vec<(StepKeyword, String)>> {
    let mut files = Vec::new();
    for path in paths {
        gather_features(path, &mut files)?;
    }
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

fn gather_features(path: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    let meta = fs::metadata(path)?;
    if meta.is_file() {
        if path.extension().is_some_and(|e| e == "feature") {
            out.push(path.to_path_buf());
        }
        return Ok(());
    }
    let mut dirs = VecDeque::from([path.to_path_buf()]);
    while let Some(dir) = dirs.pop_front() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                dirs.push_back(path);
            } else if path.extension().is_some_and(|e| e == "feature") {
                out.push(path);
            }
        }
    }
    Ok(())
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
