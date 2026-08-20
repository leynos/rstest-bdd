//! LSP lifecycle handlers for initialization and shutdown.
//!
//! This module implements the core lifecycle protocol handlers required by
//! the LSP specification: `initialize`, `initialized`, and `shutdown`.

use std::path::{Path, PathBuf};

use async_lsp::ClientSocket;
use async_lsp::ResponseError;
use lsp_types::{InitializeParams, InitializeResult, InitializedParams, ServerInfo, Url};
use tracing::{info, warn};

use crate::discovery::{WorkspaceInfo, discover_workspace};
use crate::error::ServerError;
use crate::indexing::WorkspaceRoot;
use crate::server::{ServerState, build_server_capabilities};

/// Outcome of the synchronous part of handling an `initialize` request.
///
/// The initialize path is asynchronous: the non-blocking parameter handling
/// below runs on the router task, while workspace discovery and capability
/// opening run via [`tokio::task::spawn_blocking`] and are applied to
/// `ServerState` through a [`WorkspaceReadyEvent`] once the blocking work
/// completes.
#[derive(Debug)]
pub struct InitializeOutcome {
    /// Workspace path selected from the configuration root, workspace
    /// folders, or root URI, when one exists.
    pub workspace_path: Option<PathBuf>,
    /// Initialize result returned to the client.
    pub result: InitializeResult,
}

/// Handle the synchronous part of the `initialize` request from the client.
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
pub fn handle_initialise(
    state: &mut ServerState,
    params: InitializeParams,
) -> Result<InitializeOutcome, ResponseError> {
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

    let workspace_path = state
        .config()
        .workspace_root
        .clone()
        .or_else(|| extract_workspace_path(state.workspace_folders(), root_uri.as_ref()));

    Ok(InitializeOutcome {
        workspace_path,
        result: initialize_result(),
    })
}

/// Build the `initialize` response advertised to the client.
#[must_use]
pub fn initialize_result() -> InitializeResult {
    InitializeResult {
        capabilities: build_server_capabilities(),
        server_info: Some(ServerInfo {
            name: "rstest-bdd-lsp".to_string(),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
        }),
    }
}

/// Result of the blocking workspace discovery and capability opening.
#[derive(Debug)]
pub enum WorkspacePreparation {
    /// Cargo discovery and root capability opening both succeeded.
    Discovered(WorkspaceInfo, WorkspaceRoot),
    /// Cargo discovery failed, but the requested path was opened as the root.
    DiscoveryFailed {
        root: WorkspaceRoot,
        discovery_error: ServerError,
    },
    /// Cargo discovery succeeded, but the capability could not be opened.
    RootOpenFailed(ServerError),
    /// Both cargo discovery and root opening failed.
    DiscoveryAndRootOpenFailed {
        discovery_error: ServerError,
        root_error: ServerError,
    },
}

/// Event emitted after blocking workspace preparation completes, carrying the
/// preparation result for the router task to apply to `ServerState`.
pub struct WorkspaceReadyEvent {
    /// Blocking discovery and capability-open result for the router to apply.
    pub preparation: WorkspacePreparation,
}

/// Perform blocking workspace discovery and capability opening off the LSP
/// executor.
///
/// # Errors
///
/// The function returns `Ok` for every discoverable outcome; failures are
/// encoded in [`WorkspacePreparation`] so that the resulting warnings are
/// non-fatal, matching the initialize contract. A `JoinError` from the
/// blocking task itself is handled by the caller.
pub fn prepare_workspace(path: &Path) -> WorkspacePreparation {
    let discovery = discover_workspace(path);
    let root = WorkspaceRoot::open(discovery.as_ref().map_or(path, |info| info.root.as_path()));

    match (discovery, root) {
        (Ok(info), Ok(root)) => WorkspacePreparation::Discovered(info, root),
        (Ok(_), Err(root_error)) => WorkspacePreparation::RootOpenFailed(root_error),
        (Err(discovery_error), Ok(root)) => WorkspacePreparation::DiscoveryFailed {
            root,
            discovery_error,
        },
        (Err(discovery_error), Err(root_error)) => {
            WorkspacePreparation::DiscoveryAndRootOpenFailed {
                discovery_error,
                root_error,
            }
        }
    }
}

/// Apply a completed workspace preparation to server state.
///
/// This runs on the router task and only installs capabilities that were
/// already opened off the executor, so it never blocks.
pub fn handle_workspace_ready(state: &mut ServerState, event: WorkspaceReadyEvent) {
    match event.preparation {
        WorkspacePreparation::Discovered(info, root) => {
            info!(
                root = %info.root.display(),
                packages = ?info.packages,
                "discovered workspace"
            );
            state.install_workspace_info_with_root(info, root);
        }
        WorkspacePreparation::DiscoveryFailed {
            root,
            discovery_error,
        } => {
            warn!(error = %discovery_error, "workspace discovery failed");
            state.install_workspace_root(root);
        }
        WorkspacePreparation::RootOpenFailed(root_error) => {
            warn!(error = %root_error, "failed to open workspace root capability");
        }
        WorkspacePreparation::DiscoveryAndRootOpenFailed {
            discovery_error,
            root_error,
        } => {
            warn!(error = %discovery_error, "workspace discovery failed");
            warn!(error = %root_error, "failed to open workspace root capability");
        }
    }
}

/// Complete an `initialize` request asynchronously.
///
/// Runs workspace discovery and capability opening via
/// [`tokio::task::spawn_blocking`], emits a [`WorkspaceReadyEvent`] for the
/// router task to install into `ServerState`, and returns the initialize
/// result. Workspace discovery and capability-opening failures remain
/// non-fatal and only log warnings; a task-level [`tokio::task::JoinError`]
/// is logged as a workspace-initialization failure.
///
/// # Errors
///
/// Returns a protocol error when [`handle_initialise`] could not construct an
/// initialize response.
pub async fn initialize_async(
    initialize: Result<InitializeOutcome, ResponseError>,
    client: Option<ClientSocket>,
) -> Result<InitializeResult, ResponseError> {
    let outcome = initialize?;
    if let Some(path) = outcome.workspace_path {
        match tokio::task::spawn_blocking(move || prepare_workspace(&path)).await {
            Ok(preparation) => {
                if let Some(client) = client {
                    let _ = client.emit(WorkspaceReadyEvent { preparation });
                }
            }
            Err(join_error) => {
                warn!(error = %join_error, "workspace initialization task failed");
            }
        }
    }
    Ok(outcome.result)
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
/// * `_params` - Initialized notification parameters (currently unused)
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
mod tests;
