//! Tests for bounded saved-document indexing metrics.

use super::*;

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
    });

    assert_eq!(recorder.count("feature", "success"), 1);
    assert_eq!(recorder.count("feature", "workspace-boundary-failure"), 1);
    assert_eq!(recorder.count("rust", "success"), 1);
    assert_eq!(recorder.count("rust", "recoverable-diagnostic"), 1);
    let counters = recorder.registered_counters();
    assert_eq!(counters.len(), 4);
    for counter in counters {
        assert_eq!(counter.name, INDEXING_COUNTER);
        assert_eq!(counter.labels.len(), 2);
        assert!(counter.labels.iter().any(|(key, _)| key == "operation"));
        assert!(counter.labels.iter().any(|(key, _)| key == "outcome"));
    }
}
