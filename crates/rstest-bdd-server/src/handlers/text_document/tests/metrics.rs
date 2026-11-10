//! Tests for bounded saved-document indexing metrics.

use std::path::Path;

use proc_macro2::Span;

use super::*;
use crate::indexing::{IndexedScenarioBinding, RustStepIndexError, ScenarioBindingTarget};

#[test]
fn records_indexing_outcomes_with_bounded_labels() {
    assert_eq!(INDEXING_COUNTER, "rstest_bdd_server_indexing_total");
    let workspace = TempDir::new().expect("workspace directory");
    let outside_workspace = TempDir::new().expect("outside workspace directory");
    let mut state = ServerState::new(ServerConfig::default());
    state
        .set_workspace_info(WorkspaceInfo {
            root: workspace.path().to_path_buf(),
            packages: Vec::new(),
        })
        .expect("configure workspace root");
    let recorder = IndexingRecorder::default();

    with_local_recorder(&recorder, || {
        handle_did_save_text_document(
            &mut state,
            did_save_params(
                &workspace.path().join("success.feature"),
                Some("Feature: metrics\n  Scenario: test\n    Given a step\n"),
            ),
        );
        handle_did_save_text_document(
            &mut state,
            did_save_params(&outside_workspace.path().join("outside.feature"), None),
        );
        handle_did_save_text_document(
            &mut state,
            did_save_params(
                &workspace.path().join("steps.rs"),
                Some(concat!(
                    "#[given(\"first\")]\n",
                    "#[when(\"second\")]\n",
                    "fn conflicting_step() {}\n\n",
                    "#[given(\"valid\")]\n",
                    "fn valid_step() {}\n",
                )),
            ),
        );
        handle_did_save_text_document(
            &mut state,
            did_save_params(
                &workspace.path().join("binding.rs"),
                Some("#[scenario(libraries = [accounts])]\nfn bind() {}\n"),
            ),
        );
    });

    assert_eq!(recorder.count("feature", "success"), 1);
    assert_eq!(recorder.count("feature", "workspace-boundary-failure"), 1);
    assert_eq!(recorder.count("rust", "success"), 2);
    assert_eq!(recorder.count("rust", "recoverable-diagnostic"), 1);
    assert_eq!(recorder.count("scenario-binding", "missing-path"), 1);
    let counters = recorder.registered_counters();
    assert_eq!(counters.len(), 6);
    for counter in counters {
        assert_eq!(counter.name, INDEXING_COUNTER);
        assert_eq!(counter.labels.len(), 2);
        assert!(counter.labels.iter().any(|(key, _)| key == "operation"));
        assert!(counter.labels.iter().any(|(key, _)| key == "outcome"));
    }
}

#[test]
fn clears_stale_scenario_bindings_after_rust_index_failure() {
    let rust_path = Path::new("/workspace/src/bindings.rs");
    let feature_path = Path::new("/workspace/tests/features/accounts.feature");
    let mut state = ServerState::new(ServerConfig::default());
    state.upsert_rust_scenario_bindings(
        rust_path,
        vec![IndexedScenarioBinding {
            target: ScenarioBindingTarget::Feature(feature_path.to_path_buf()),
            libraries: vec![String::from("accounts")],
        }],
    );
    assert!(
        state
            .feature_selects_library(feature_path, "accounts")
            .expect("indexed scope should select accounts")
    );

    apply_rust_source_index_result(
        &mut state,
        rust_path,
        Err(RustStepIndexError::Parse(syn::Error::new(
            Span::call_site(),
            "invalid Rust source",
        ))),
        FeatureDiagnosticPublication::DeferredReplay,
    );

    assert!(
        !state
            .feature_selects_library(feature_path, "accounts")
            .expect("failed indexing should clear stale scope")
    );
}
