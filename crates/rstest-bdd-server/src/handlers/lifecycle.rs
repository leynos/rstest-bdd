//! LSP lifecycle handlers for initialisation and shutdown.
//!
//! This module implements the core lifecycle protocol handlers required by
//! the LSP specification: `initialize`, `initialized`, and `shutdown`.

use std::path::PathBuf;

use async_lsp::ResponseError;
use lsp_types::{InitializeParams, InitializeResult, InitializedParams, ServerInfo, Url};
use tracing::{info, warn};

use crate::discovery::discover_workspace;
use crate::error::ServerError;
use crate::server::{build_server_capabilities, ServerState};

/// Handle the `initialize` request from the client.
///
/// This handler validates client capabilities, discovers workspace roots,
/// and returns the server's capabilities. Per the LSP specification, this
/// must be the first request sent by the client.
///
/// # Arguments
///
/// * `state` - Mutable reference to the server state
/// * `params` - Initialisation parameters from the client
///
/// # Errors
///
/// Returns a `ResponseError` if:
/// - The server is already initialised
/// - Workspace discovery fails (logged as warning, does not fail the request)
pub fn handle_initialise(
    state: &mut ServerState,
    params: InitializeParams,
) -> Result<InitializeResult, ResponseError> {
    if state.is_initialised() {
        return Err(response_error(
            &ServerError::AlreadyInitialised,
            async_lsp::ErrorCode::INVALID_REQUEST,
        ));
    }

    // Store client capabilities
    state.client_capabilities = Some(params.capabilities);

    // Store workspace folders if provided
    if let Some(folders) = params.workspace_folders {
        state.workspace_folders = folders;
    }

    // Attempt workspace discovery from workspace folders
    let workspace_path = extract_workspace_path(&state.workspace_folders);
    if let Some(path) = workspace_path {
        match discover_workspace(&path) {
            Ok(info) => {
                info!(
                    root = %info.root.display(),
                    packages = ?info.packages,
                    "discovered workspace"
                );
                state.workspace_info = Some(info);
            }
            Err(e) => {
                warn!(error = %e, "workspace discovery failed");
            }
        }
    }

    Ok(InitializeResult {
        capabilities: build_server_capabilities(),
        server_info: Some(ServerInfo {
            name: "rstest-bdd-lsp".to_string(),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
        }),
    })
}

/// Handle the `initialized` notification from the client.
///
/// This notification signals that the client has processed the initialize
/// response and is ready for normal operation. The server marks itself as
/// fully initialised at this point.
///
/// # Arguments
///
/// * `state` - Mutable reference to the server state
/// * `_params` - Initialised notification parameters (currently unused)
pub fn handle_initialised(state: &mut ServerState, _params: InitializedParams) {
    state.mark_initialised();
    info!("server initialised");
}

/// Handle the `shutdown` request from the client.
///
/// This request signals that the client is about to exit and the server
/// should prepare for termination. Per the LSP specification, the server
/// should not exit until it receives the `exit` notification.
///
/// # Arguments
///
/// * `_state` - Reference to the server state (currently unused)
///
/// # Errors
///
/// Currently always returns `Ok(())`. Future implementations may return
/// errors if cleanup operations fail.
pub fn handle_shutdown(_state: &mut ServerState) -> Result<(), ResponseError> {
    info!("shutdown request received");
    Ok(())
}

/// Extract a workspace path from workspace folders.
///
/// Returns the path of the first workspace folder with a file:// scheme.
fn extract_workspace_path(workspace_folders: &[lsp_types::WorkspaceFolder]) -> Option<PathBuf> {
    workspace_folders.first().and_then(|f| url_to_path(&f.uri))
}

/// Convert a URL to a file system path.
///
/// Only handles `file://` URLs; returns `None` for other schemes.
fn url_to_path(url: &Url) -> Option<PathBuf> {
    url.to_file_path().ok()
}

/// Convert a server error to an LSP response error.
fn response_error(err: &ServerError, code: async_lsp::ErrorCode) -> ResponseError {
    ResponseError::new(code, err.to_string())
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "tests require explicit panic messages for debugging failures"
)]
mod tests {
    use super::*;
    use crate::config::ServerConfig;
    use lsp_types::ClientCapabilities;
    use std::str::FromStr;

    fn create_test_state() -> ServerState {
        ServerState::new(ServerConfig::default())
    }

    fn create_init_params() -> InitializeParams {
        InitializeParams {
            capabilities: ClientCapabilities::default(),
            workspace_folders: None,
            ..Default::default()
        }
    }

    #[test]
    fn handle_initialise_stores_client_capabilities() {
        let mut state = create_test_state();
        let params = create_init_params();

        let result = handle_initialise(&mut state, params);

        assert!(result.is_ok());
        assert!(state.client_capabilities.is_some());
    }

    #[test]
    fn handle_initialise_returns_server_info() {
        let mut state = create_test_state();
        let params = create_init_params();

        let result = handle_initialise(&mut state, params);
        let init_result = result.expect("initialisation should succeed");

        assert!(init_result.server_info.is_some());
        let info = init_result.server_info.expect("should have server info");
        assert_eq!(info.name, "rstest-bdd-lsp");
        assert!(info.version.is_some());
    }

    #[test]
    fn handle_initialise_fails_when_already_initialised() {
        let mut state = create_test_state();
        state.mark_initialised();
        let params = create_init_params();

        let result = handle_initialise(&mut state, params);

        assert!(result.is_err());
    }

    #[test]
    fn handle_initialised_marks_state_as_initialised() {
        let mut state = create_test_state();
        assert!(!state.is_initialised());

        handle_initialised(&mut state, InitializedParams {});

        assert!(state.is_initialised());
    }

    #[test]
    fn handle_shutdown_returns_ok() {
        let mut state = create_test_state();

        let result = handle_shutdown(&mut state);

        assert!(result.is_ok());
    }

    #[test]
    fn url_to_path_handles_file_url() {
        // Use a platform-appropriate test path
        #[cfg(windows)]
        let test_path = PathBuf::from("C:\\test\\path");
        #[cfg(not(windows))]
        let test_path = PathBuf::from("/test/path");

        let url = Url::from_file_path(&test_path).expect("valid path");
        let path = url_to_path(&url);

        assert!(path.is_some());
        assert_eq!(path.expect("should have path"), test_path);
    }

    #[test]
    fn url_to_path_returns_none_for_non_file_url() {
        let url = Url::from_str("https://example.com/path").expect("valid URL");
        let path = url_to_path(&url);

        assert!(path.is_none());
    }

    #[test]
    fn extract_workspace_path_from_folders() {
        // Use a platform-appropriate test path
        #[cfg(windows)]
        let test_path = PathBuf::from("C:\\folder\\path");
        #[cfg(not(windows))]
        let test_path = PathBuf::from("/folder/path");

        let folders = vec![lsp_types::WorkspaceFolder {
            uri: Url::from_file_path(&test_path).expect("valid path"),
            name: "folder".to_string(),
        }];

        let path = extract_workspace_path(&folders);

        assert!(path.is_some());
        assert_eq!(path.expect("should have path"), test_path);
    }

    #[test]
    fn extract_workspace_path_returns_none_when_empty() {
        let path = extract_workspace_path(&[]);

        assert!(path.is_none());
    }
}
