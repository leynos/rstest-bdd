//! Publication of conflicting closed step-library scopes.
//!
//! Scope resolution is a pure query: it reports a [`ScenarioScopeConflict`] as
//! data and never logs. Logging a conflict from a query would make
//! `textDocument/definition`, `textDocument/implementation`, and the diagnostic
//! computations emit an external side-effect for every read, so one unresolved
//! conflict would produce a burst of duplicate warnings.
//!
//! This module is the single publication boundary. An indexing mutation asks
//! [`ScopeAlerts::take_unreported`] whether the conflict it just observed has
//! already been announced; only the first observation per
//! `(feature file, conflicting scope set)` pair produces a warning. A feature
//! that stops conflicting forgets its pair, so resolving and then reintroducing
//! the same disagreement reports it again rather than staying silent.
//!
//! Both index mutations publish, because either can be the one that completes
//! the picture: a feature save can arrive after its bindings, and a binding save
//! can arrive after its feature.

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use tracing::warn;

use super::scenario_scopes::ScenarioScopeConflict;

/// One conflict identity that is worth announcing at most once.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ScopeAlertKey {
    /// Feature file whose bindings disagree.
    feature_path: PathBuf,
    /// Distinct normalized closed scopes selected for the feature.
    scopes: Vec<Vec<String>>,
}

impl ScopeAlertKey {
    /// Return the feature file the conflict belongs to.
    pub(crate) fn feature_path(&self) -> &Path { &self.feature_path }

    /// Return the conflicting scope sets in registry order.
    pub(crate) fn scopes(&self) -> &[Vec<String>] { &self.scopes }
}

/// Remembers which conflicts have already been announced.
#[derive(Debug, Default)]
pub(super) struct ScopeAlerts {
    /// Conflict identities that produced a warning.
    reported: HashSet<ScopeAlertKey>,
}

impl ScopeAlerts {
    /// Return the conflict identity when it has not been announced yet.
    ///
    /// Taking the key records it, so a second observation of the same conflict
    /// returns `None`. This is the whole query path: no logging happens here.
    pub(super) fn take_unreported(
        &mut self,
        conflict: &ScenarioScopeConflict,
    ) -> Option<ScopeAlertKey> {
        let key = ScopeAlertKey {
            feature_path: conflict.feature_path().to_path_buf(),
            scopes: conflict.scopes().to_vec(),
        };
        self.reported.insert(key.clone()).then_some(key)
    }

    /// Forget every conflict recorded for one feature file.
    ///
    /// Called when the feature resolves to a single scope again, so a later
    /// reintroduction of the same disagreement is reported rather than
    /// suppressed by state that no longer describes the index.
    pub(super) fn forget(&mut self, feature_path: &Path) {
        self.reported
            .retain(|key| key.feature_path() != feature_path);
    }
}

impl super::ServerState {
    /// Announce a first-time scope conflict for one feature at a mutation boundary.
    ///
    /// A feature that no longer conflicts forgets its history, so resolving and
    /// then reintroducing the same disagreement reports it again rather than
    /// staying silent because of a stale record.
    pub(crate) fn report_scope_conflict(&mut self, feature_path: &Path) {
        match self.scenario_scopes.libraries_for_feature(feature_path) {
            Err(conflict) => {
                if let Some(key) = self.scope_alerts.take_unreported(&conflict) {
                    warn!(
                        operation = "resolve-feature-step-scope",
                        feature_path = %key.feature_path().display(),
                        selected_scopes = ?key.scopes(),
                        failure_category = "conflicting-closed-scopes",
                        fallback_state = "no-step-candidates",
                        "scenario bindings select conflicting closed step-library scopes"
                    );
                }
            }
            Ok(_) => self.scope_alerts.forget(feature_path),
        }
    }

    /// Announce conflicts for every indexed feature after a binding mutation.
    ///
    /// A Rust save can create or remove a conflict for any feature, and a
    /// `scenarios!` directory target cannot be inverted into a feature list, so
    /// the mutation reports for the indexed set. This matches the existing
    /// `publish_all_feature_diagnostics` call made on the same path.
    pub(crate) fn report_indexed_scope_conflicts(&mut self) {
        let feature_paths: Vec<_> = self.feature_indices.keys().cloned().collect();
        for feature_path in feature_paths {
            self.report_scope_conflict(&feature_path);
        }
    }
}
