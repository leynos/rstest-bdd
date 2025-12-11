//! Core language server state and service construction.
//!
//! This module defines the central state shared across all LSP handlers and
//! provides the service construction for the language server.

use lsp_types::{ClientCapabilities, ServerCapabilities, WorkspaceFolder};

use crate::config::ServerConfig;
use crate::discovery::WorkspaceInfo;

/// Central state shared across all LSP handlers.
///
/// This struct holds the in-memory state of the language server, including
/// the workspace configuration and any cached data. It is passed to handlers
/// via the async-lsp router.
#[derive(Debug)]
pub struct ServerState {
    /// Client capabilities received during initialisation.
    pub client_capabilities: Option<ClientCapabilities>,
    /// Discovered workspace information.
    pub workspace_info: Option<WorkspaceInfo>,
    /// Workspace folders from the client.
    pub workspace_folders: Vec<WorkspaceFolder>,
    /// Whether the server has been initialised.
    pub initialised: bool,
    /// Configuration loaded from environment and client.
    pub config: ServerConfig,
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
        }
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
}

/// Build the server capabilities to advertise to the client.
///
/// Currently returns minimal capabilities as Phase 7 focuses on scaffolding.
/// Future phases will add navigation and diagnostic capabilities.
#[must_use]
pub fn build_server_capabilities() -> ServerCapabilities {
    ServerCapabilities::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_state_is_not_initialised() {
        let config = ServerConfig::default();
        let state = ServerState::new(config);
        assert!(!state.is_initialised());
        assert!(state.client_capabilities.is_none());
        assert!(state.workspace_info.is_none());
        assert!(state.workspace_folders.is_empty());
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
        // Phase 7 returns minimal capabilities
        assert!(capabilities.text_document_sync.is_none());
        assert!(capabilities.definition_provider.is_none());
    }
}
