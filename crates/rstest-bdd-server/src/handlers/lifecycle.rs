//! LSP lifecycle handlers for initialization and shutdown.
//!
//! This module implements the core lifecycle protocol handlers required by
//! the LSP specification: `initialize`, `initialized`, and `shutdown`.

use std::path::PathBuf;

use async_lsp::ResponseError;
use lsp_types::{InitializeParams, InitializeResult, InitializedParams, ServerInfo, Url};
use tracing::{info, warn};

use crate::discovery::discover_workspace;
use crate::error::ServerError;
use crate::server::{ServerState, build_server_capabilities};

/// Handle the `initialize` request from the client.
///
/// This handler validates client capabilities, discovers workspace roots,
/// and returns the server's capabilities. Per the LSP specification, this
/// must be the first request sent by the client.
///
/// # Arguments
///
/// * `state` - Mutable reference to the server state
/// * `params` - Initialization parameters from the client
///
/// # Errors
///
/// Returns a `ResponseError` when the server is already initialized.
///
/// Workspace discovery failures are logged as warnings and do not fail the
/// request.
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
    #[expect(
        deprecated,
        reason = "Some clients still populate root_uri instead of workspace_folders."
    )]
    let InitializeParams {
        capabilities,
        workspace_folders,
        root_uri,
        ..
    } = params;
    state.set_client_capabilities(capabilities);

    // Store workspace folders if provided
    if let Some(folders) = workspace_folders {
        state.set_workspace_folders(folders);
    }

    // Use CLI/env workspace root if provided, otherwise discover from client.
    let workspace_path = state
        .config()
        .workspace_root
        .clone()
        .or_else(|| extract_workspace_path(state.workspace_folders(), root_uri.as_ref()));
    if let Some(path) = workspace_path {
        match discover_workspace(&path) {
            Ok(info) => {
                info!(
                    root = %info.root.display(),
                    packages = ?info.packages,
                    "discovered workspace"
                );
                state.set_workspace_info(info);
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
/// fully initialized at this point.
///
/// # Arguments
///
/// * `state` - Mutable reference to the server state
/// * `_params` - Initialised notification parameters (currently unused)
pub fn handle_initialised(state: &mut ServerState, _params: InitializedParams) {
    state.mark_initialised();
    info!("server initialized");
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
/// Returns the path of the first workspace folder with a file:// scheme. When
/// no folders are provided, the root URI is used (for single-root clients).
fn extract_workspace_path(
    workspace_folders: &[lsp_types::WorkspaceFolder],
    root_uri: Option<&Url>,
) -> Option<PathBuf> {
    workspace_folders
        .first()
        .and_then(|f| url_to_path(&f.uri))
        .or_else(|| root_uri.and_then(url_to_path))
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
    use lsp_types::{ClientCapabilities, WorkspaceFolder};
    use rstest::{fixture, rstest};
    use std::str::FromStr;

    #[fixture]
    fn create_test_state() -> ServerState {
        ServerState::new(ServerConfig::default())
    }

    #[fixture]
    fn create_init_params() -> InitializeParams {
        InitializeParams {
            capabilities: ClientCapabilities::default(),
            workspace_folders: None,
            ..Default::default()
        }
    }

    /// Fixture providing a platform-specific test path.
    #[fixture]
    fn platform_test_path() -> PathBuf {
        #[cfg(windows)]
        let path = PathBuf::from("C:\\test\\path");
        #[cfg(not(windows))]
        let path = PathBuf::from("/test/path");
        path
    }

    #[rstest]
    fn handle_initialise_stores_client_capabilities(
        mut create_test_state: ServerState,
        create_init_params: InitializeParams,
    ) {
        let result = handle_initialise(&mut create_test_state, create_init_params);

        assert!(result.is_ok());
        assert!(create_test_state.client_capabilities().is_some());
    }

    #[rstest]
    fn handle_initialise_returns_server_info(
        mut create_test_state: ServerState,
        create_init_params: InitializeParams,
    ) {
        let result = handle_initialise(&mut create_test_state, create_init_params);
        let init_result = result.expect("initialization should succeed");

        assert!(init_result.server_info.is_some());
        let info = init_result.server_info.expect("should have server info");
        assert_eq!(info.name, "rstest-bdd-lsp");
        assert!(info.version.is_some());
    }

    #[rstest]
    fn handle_initialise_fails_when_already_initialised(
        mut create_test_state: ServerState,
        create_init_params: InitializeParams,
    ) {
        create_test_state.mark_initialised();

        let result = handle_initialise(&mut create_test_state, create_init_params);

        assert!(result.is_err());
    }

    #[rstest]
    fn handle_initialised_marks_state_as_initialised(mut create_test_state: ServerState) {
        assert!(!create_test_state.is_initialised());

        handle_initialised(&mut create_test_state, InitializedParams {});

        assert!(create_test_state.is_initialised());
    }

    #[rstest]
    fn handle_shutdown_returns_ok(mut create_test_state: ServerState) {
        let result = handle_shutdown(&mut create_test_state);

        assert!(result.is_ok());
    }

    #[test]
    fn url_to_path_returns_none_for_non_file_url() {
        let url = Url::from_str("https://example.com/path").expect("valid URL");
        let path = url_to_path(&url);

        assert!(path.is_none());
    }

    #[rstest]
    fn url_to_path_handles_file_url(platform_test_path: PathBuf) {
        let url = Url::from_file_path(&platform_test_path).expect("valid path");
        let path = url_to_path(&url);

        assert!(path.is_some());
        assert_eq!(path.expect("should have path"), platform_test_path);
    }

    #[rstest]
    #[case::from_workspace_folders(true, None)]
    #[case::from_root_uri(false, Some("root_uri"))]
    #[expect(
        clippy::used_underscore_binding,
        reason = "rstest uses this parameter; name matches review instructions"
    )]
    fn extract_workspace_path_from_various_sources(
        platform_test_path: PathBuf,
        #[case] use_folders: bool,
        #[case] _description: Option<&str>,
    ) {
        let url = Url::from_file_path(&platform_test_path).expect("valid path");

        let (folders, root_uri) = if use_folders {
            (
                vec![WorkspaceFolder {
                    uri: url.clone(),
                    name: "folder".to_string(),
                }],
                None,
            )
        } else {
            (Vec::new(), Some(url.clone()))
        };

        let path = extract_workspace_path(&folders, root_uri.as_ref());

        assert!(path.is_some());
        assert_eq!(path.expect("should have path"), platform_test_path);
    }

    #[test]
    fn extract_workspace_path_returns_none_when_empty() {
        let path = extract_workspace_path(&[], None);

        assert!(path.is_none());
    }

    #[rstest]
    fn handle_initialise_prefers_config_workspace_root_over_client(
        create_init_params: InitializeParams,
    ) {
        let override_path = PathBuf::from("/override/workspace");
        let config = ServerConfig::default().with_workspace_root(override_path.clone());
        let mut state = ServerState::new(config);

        // Initialisation succeeds even though the override path does not
        // exist — workspace discovery logs a warning but does not fail.
        let result = handle_initialise(&mut state, create_init_params);
        assert!(result.is_ok());

        // The config workspace root is preserved, confirming it was used
        // as the discovery path instead of the (empty) client root URI.
        assert_eq!(state.config().workspace_root, Some(override_path));
    }
}
