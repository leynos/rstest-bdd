//! Core language server state and service construction.
//!
//! This module defines the central state shared across all LSP handlers and
//! provides the service construction for the language server.

use std::collections::HashMap;
use std::path::Path;

use lsp_types::{ClientCapabilities, ServerCapabilities, WorkspaceFolder};
use lsp_types::{TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions};

use crate::config::ServerConfig;
use crate::discovery::WorkspaceInfo;
use crate::indexing::FeatureFileIndex;

/// Central state shared across all LSP handlers.
///
/// This struct holds the in-memory state of the language server, including
/// the workspace configuration and any cached data. It is passed to handlers
/// via the async-lsp router.
#[derive(Debug)]
pub struct ServerState {
    /// Client capabilities received during initialisation.
    client_capabilities: Option<ClientCapabilities>,
    /// Discovered workspace information.
    workspace_info: Option<WorkspaceInfo>,
    /// Workspace folders from the client.
    workspace_folders: Vec<WorkspaceFolder>,
    /// Whether the server has been initialised.
    initialised: bool,
    /// Configuration loaded from environment and client.
    config: ServerConfig,
    /// Indexed `.feature` files keyed by absolute path.
    feature_indices: HashMap<std::path::PathBuf, FeatureFileIndex>,
}

impl ServerState {
    /// Create a new server state with the given configuration.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd_server::config::ServerConfig;
    /// use rstest_bdd_server::server::ServerState;
    ///
    /// let config = ServerConfig::default();
    /// let state = ServerState::new(config);
    /// assert!(!state.initialised);
    /// ```
    #[must_use]
    pub fn new(config: ServerConfig) -> Self {
        Self {
            client_capabilities: None,
            workspace_info: None,
            workspace_folders: Vec::new(),
            initialised: false,
            config,
            feature_indices: HashMap::new(),
        }
    }

    /// Store client capabilities received during initialization.
    pub fn set_client_capabilities(&mut self, capabilities: ClientCapabilities) {
        self.client_capabilities = Some(capabilities);
    }

    /// Access the stored client capabilities, if any.
    #[must_use]
    pub fn client_capabilities(&self) -> Option<&ClientCapabilities> {
        self.client_capabilities.as_ref()
    }

    /// Store workspace folders provided by the client.
    pub fn set_workspace_folders(&mut self, folders: Vec<WorkspaceFolder>) {
        self.workspace_folders = folders;
    }

    /// Access the workspace folders provided by the client.
    #[must_use]
    pub fn workspace_folders(&self) -> &[WorkspaceFolder] {
        &self.workspace_folders
    }

    /// Store discovered workspace information.
    pub fn set_workspace_info(&mut self, workspace_info: WorkspaceInfo) {
        self.workspace_info = Some(workspace_info);
    }

    /// Access discovered workspace information, if available.
    #[must_use]
    pub fn workspace_info(&self) -> Option<&WorkspaceInfo> {
        self.workspace_info.as_ref()
    }

    /// Access the current server configuration.
    #[must_use]
    pub fn config(&self) -> &ServerConfig {
        &self.config
    }

    /// Mark the server as initialised.
    pub fn mark_initialised(&mut self) {
        self.initialised = true;
    }

    /// Check if the server is initialised.
    #[must_use]
    pub fn is_initialised(&self) -> bool {
        self.initialised
    }

    /// Insert or update the cached index for a `.feature` file.
    pub fn upsert_feature_index(&mut self, index: FeatureFileIndex) {
        self.feature_indices.insert(index.path.clone(), index);
    }

    /// Retrieve the cached index for a `.feature` file, if present.
    #[must_use]
    pub fn feature_index(&self, path: &Path) -> Option<&FeatureFileIndex> {
        self.feature_indices.get(path)
    }
}

/// Build the server capabilities to advertise to the client.
///
/// Phase 7 advertises text document sync to receive save notifications for
/// `.feature` file indexing. Navigation and diagnostics are added in later
/// phases.
#[must_use]
pub fn build_server_capabilities() -> ServerCapabilities {
    ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Options(
            TextDocumentSyncOptions {
                open_close: Some(true),
                change: Some(TextDocumentSyncKind::INCREMENTAL),
                save: Some(lsp_types::TextDocumentSyncSaveOptions::SaveOptions(
                    lsp_types::SaveOptions {
                        include_text: Some(false),
                    },
                )),
                ..Default::default()
            },
        )),
        ..ServerCapabilities::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_state_is_not_initialised() {
        let config = ServerConfig::default();
        let state = ServerState::new(config);
        assert!(!state.is_initialised());
        assert!(state.client_capabilities().is_none());
        assert!(state.workspace_info().is_none());
        assert!(state.workspace_folders().is_empty());
    }

    #[test]
    fn mark_initialised_sets_flag() {
        let config = ServerConfig::default();
        let mut state = ServerState::new(config);
        state.mark_initialised();
        assert!(state.is_initialised());
    }

    #[test]
    fn build_server_capabilities_returns_default() {
        let capabilities = build_server_capabilities();
        assert!(capabilities.text_document_sync.is_some());
        assert!(capabilities.definition_provider.is_none());
    }
}
