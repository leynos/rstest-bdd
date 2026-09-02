//! Resolves feature files to the closed step-library scopes in Rust bindings.

use std::{
    collections::{BTreeSet, HashMap},
    path::{Path, PathBuf},
    sync::Arc,
};

use gherkin::StepType;
use tracing::warn;

use super::ServerState;
use crate::indexing::{CompiledStepDefinition, IndexedScenarioBinding, ScenarioBindingTarget};

/// Scenario bindings grouped by the Rust source file that declared them.
#[derive(Debug, Default)]
pub(super) struct ScenarioScopeRegistry {
    /// Bindings replaced atomically when one Rust file is re-indexed.
    bindings_by_file: HashMap<PathBuf, Vec<IndexedScenarioBinding>>,
}

/// Conflicting closed scopes selected by bindings for one feature file.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("feature {} has conflicting step-library scopes: {scopes:?}", feature_path.display())]
pub(crate) struct ScenarioScopeConflict {
    /// Feature file whose bindings disagree.
    feature_path: PathBuf,
    /// Distinct normalized closed scopes selected for the feature.
    scopes: Vec<Vec<String>>,
}

impl ScenarioScopeConflict {
    /// Emit the conservative no-candidate fallback as a structured warning.
    pub(crate) fn emit_warning(&self) {
        warn!(
            operation = "resolve-feature-step-scope",
            feature_path = %self.feature_path.display(),
            selected_scopes = ?self.scopes,
            failure_category = "conflicting-closed-scopes",
            fallback_state = "no-step-candidates",
            "scenario bindings select conflicting closed step-library scopes"
        );
    }
}

impl ScenarioScopeRegistry {
    /// Replace every scenario binding originating from one Rust source file.
    pub(super) fn replace_rust_file(&mut self, path: &Path, bindings: Vec<IndexedScenarioBinding>) {
        if bindings.is_empty() {
            self.bindings_by_file.remove(path);
        } else {
            self.bindings_by_file.insert(path.to_path_buf(), bindings);
        }
    }

    /// Return the single closed scope selected for one feature file.
    fn libraries_for_feature(
        &self,
        feature_path: &Path,
    ) -> Result<Option<Vec<String>>, ScenarioScopeConflict> {
        let scopes: BTreeSet<_> = self
            .matching_bindings(feature_path)
            .map(|binding| normalized_scope(&binding.libraries))
            .collect();
        if scopes.len() > 1 {
            return Err(ScenarioScopeConflict {
                feature_path: feature_path.to_path_buf(),
                scopes: scopes.into_iter().collect(),
            });
        }
        Ok(scopes.into_iter().next())
    }

    /// Iterate over bindings whose target contains one feature file.
    fn matching_bindings<'a>(
        &'a self,
        feature_path: &'a Path,
    ) -> impl Iterator<Item = &'a IndexedScenarioBinding> {
        self.bindings_by_file
            .iter()
            .flat_map(move |(rust_path, bindings)| {
                bindings.iter().filter(move |binding| {
                    binding_matches_feature(rust_path, &binding.target, feature_path)
                })
            })
    }
}

/// Normalize a closed scope because library order carries no precedence.
fn normalized_scope(libraries: &[String]) -> Vec<String> {
    let mut normalized = libraries.to_vec();
    normalized.sort_unstable();
    normalized.dedup();
    normalized
}

impl ServerState {
    /// Replace the internal scenario bindings indexed from one Rust source.
    pub(crate) fn upsert_rust_scenario_bindings(
        &mut self,
        path: &Path,
        bindings: Vec<IndexedScenarioBinding>,
    ) {
        self.scenario_scopes.replace_rust_file(path, bindings);
    }

    /// Return keyword-compatible definitions in a feature's selected scope.
    pub(crate) fn steps_for_feature_keyword(
        &self,
        feature_path: &Path,
        keyword: StepType,
    ) -> Result<Vec<&Arc<CompiledStepDefinition>>, ScenarioScopeConflict> {
        let libraries = self
            .scenario_scopes
            .libraries_for_feature(feature_path)?
            .unwrap_or_else(|| vec![String::from("rstest_bdd::global")]);
        Ok(self
            .step_registry
            .steps_for_keyword_in_scope(keyword, &libraries))
    }

    /// Return whether a feature binding selects one definition's library.
    pub(crate) fn feature_selects_library(
        &self,
        feature_path: &Path,
        library: &str,
    ) -> Result<bool, ScenarioScopeConflict> {
        Ok(self
            .scenario_scopes
            .libraries_for_feature(feature_path)?
            .map_or(library == "rstest_bdd::global", |libraries| {
                libraries.iter().any(|selected| selected == library)
            }))
    }
}

/// Match one indexed binding relative to every ancestor of its Rust source.
fn binding_matches_feature(
    rust_path: &Path,
    target: &ScenarioBindingTarget,
    feature_path: &Path,
) -> bool {
    let Some(source_directory) = rust_path.parent() else {
        return false;
    };
    source_directory.ancestors().any(|base| match target {
        ScenarioBindingTarget::Feature(path) => resolve_target(base, path) == feature_path,
        ScenarioBindingTarget::Directory(path) => {
            feature_path.starts_with(resolve_target(base, path))
                && feature_path
                    .extension()
                    .is_some_and(|extension| extension == "feature")
        }
    })
}

/// Resolve an absolute target directly or a relative target below `base`.
fn resolve_target(base: &Path, target: &Path) -> PathBuf {
    if target.is_absolute() {
        target.to_path_buf()
    } else {
        base.join(target)
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for feature-to-library scope resolution.

    use super::*;

    #[test]
    fn resolves_feature_and_directory_bindings_from_crate_ancestors() {
        let mut scopes = ScenarioScopeRegistry::default();
        scopes.replace_rust_file(
            Path::new("/workspace/crate/src/lib.rs"),
            vec![
                IndexedScenarioBinding {
                    target: ScenarioBindingTarget::Feature(PathBuf::from(
                        "tests/features/account.feature",
                    )),
                    libraries: vec![String::from("accounts")],
                },
                IndexedScenarioBinding {
                    target: ScenarioBindingTarget::Directory(PathBuf::from("tests/features/files")),
                    libraries: vec![String::from("filesystem")],
                },
            ],
        );

        assert_eq!(
            scopes.libraries_for_feature(Path::new(
                "/workspace/crate/tests/features/account.feature"
            )),
            Ok(Some(vec![String::from("accounts")]))
        );
        assert_eq!(
            scopes.libraries_for_feature(Path::new(
                "/workspace/crate/tests/features/files/write.feature"
            )),
            Ok(Some(vec![String::from("filesystem")]))
        );
    }

    #[test]
    fn reports_conflicting_scopes_without_merging_them() {
        let feature_path = Path::new("/workspace/crate/tests/features/account.feature");
        let mut scopes = ScenarioScopeRegistry::default();
        scopes.replace_rust_file(
            Path::new("/workspace/crate/src/accounts.rs"),
            vec![IndexedScenarioBinding {
                target: ScenarioBindingTarget::Feature(PathBuf::from(
                    "tests/features/account.feature",
                )),
                libraries: vec![String::from("accounts")],
            }],
        );
        scopes.replace_rust_file(
            Path::new("/workspace/crate/src/filesystem.rs"),
            vec![IndexedScenarioBinding {
                target: ScenarioBindingTarget::Feature(PathBuf::from(
                    "tests/features/account.feature",
                )),
                libraries: vec![String::from("filesystem")],
            }],
        );

        let error = scopes
            .libraries_for_feature(feature_path)
            .expect_err("different closed scopes must remain ambiguous");

        assert_eq!(
            error.scopes,
            [
                vec![String::from("accounts")],
                vec![String::from("filesystem")]
            ]
        );
    }

    #[test]
    fn treats_reordered_library_lists_as_the_same_scope() {
        let feature_path = Path::new("/workspace/crate/tests/features/account.feature");
        let target =
            ScenarioBindingTarget::Feature(PathBuf::from("tests/features/account.feature"));
        let mut scopes = ScenarioScopeRegistry::default();
        scopes.replace_rust_file(
            Path::new("/workspace/crate/src/lib.rs"),
            vec![
                IndexedScenarioBinding {
                    target: target.clone(),
                    libraries: vec![String::from("common"), String::from("accounts")],
                },
                IndexedScenarioBinding {
                    target,
                    libraries: vec![String::from("accounts"), String::from("common")],
                },
            ],
        );

        assert_eq!(
            scopes.libraries_for_feature(feature_path),
            Ok(Some(vec![String::from("accounts"), String::from("common")]))
        );
    }
}
