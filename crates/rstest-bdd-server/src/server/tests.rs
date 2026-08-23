//! Unit tests for server state management.

use super::*;

#[test]
fn new_state_is_not_initialized() {
    let config = ServerConfig::default();
    let state = ServerState::new(config);
    assert!(!state.is_initialised());
    assert!(state.client_capabilities().is_none());
    assert!(state.workspace_info().is_none());
    assert!(state.workspace_folders().is_empty());
    assert!(state.feature_indices.is_empty());
    assert!(state.rust_step_indices.is_empty());
    assert!(
        state
            .step_registry
            .steps_for_keyword(gherkin::StepType::Given)
            .is_empty()
    );
}

#[test]
fn mark_initialized_sets_flag() {
    let config = ServerConfig::default();
    let mut state = ServerState::new(config);
    state.mark_initialised();
    assert!(state.is_initialised());
}

#[test]
fn build_server_capabilities_includes_definition_provider() {
    let capabilities = build_server_capabilities();
    assert!(capabilities.text_document_sync.is_some());
    assert!(capabilities.definition_provider.is_some());
}

#[test]
fn build_server_capabilities_includes_implementation_provider() {
    let capabilities = build_server_capabilities();
    assert!(capabilities.implementation_provider.is_some());
}
