//! Core language server state and service construction.
//!
//! This module defines the central state shared across all LSP handlers and
//! provides the service construction for the language server.

use std::collections::HashMap;
use std::path::Path;

use async_lsp::ClientSocket;
use lsp_types::{ClientCapabilities, ServerCapabilities, WorkspaceFolder};
use lsp_types::{TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions};
use tracing::warn;

use crate::config::ServerConfig;
use crate::discovery::WorkspaceInfo;
use crate::error::ServerError;
use crate::indexing::{
    FeatureFileIndex, FeatureIndexError, RustStepFileIndex, StepDefinitionRegistry, WorkspaceRoot,
    index_feature_source_owned,
};

mod deferred_saves;
mod workspace_task;

use deferred_saves::{DeferredDocumentSaves, DeferredSaveDropReason};
use workspace_task::WorkspaceTask;

/// Central state shared across all LSP handlers.
///
/// This struct holds the in-memory state of the language server, including
/// the workspace configuration and any cached data. It is passed to handlers
/// via the async-lsp router.
///
/// Note: `Debug` is manually implemented because `ClientSocket` does not
/// derive `Debug`.
pub struct ServerState {
    /// Client capabilities received during initialization.
    client_capabilities: Option<ClientCapabilities>,
    /// Discovered workspace information.
    workspace_info: Option<WorkspaceInfo>,
    /// Capability-scoped workspace root for disk-backed feature-file reads.
    workspace_root: Option<WorkspaceRoot>,
    /// Workspace folders from the client.
    workspace_folders: Vec<WorkspaceFolder>,
    /// Monotonic identifier for the workspace initialization in progress.
    workspace_initialization_id: u64,
    /// Whether a workspace capability is still being prepared.
    workspace_preparation_pending: bool,
    /// Background preparation or replay task owned by this server instance.
    workspace_task: WorkspaceTask,
    /// Saves received before the workspace capability became available.
    deferred_document_saves: DeferredDocumentSaves,
    /// Whether the server has been initialized.
    initialized: bool,
    /// Configuration loaded from environment and client.
    config: ServerConfig,
    /// Indexed `.feature` files keyed by absolute path.
    feature_indices: HashMap<std::path::PathBuf, FeatureFileIndex>,
    /// Indexed Rust step definition files keyed by absolute path.
    rust_step_indices: HashMap<std::path::PathBuf, RustStepFileIndex>,
    /// Compiled step patterns keyed by keyword, built from Rust step indices.
    step_registry: StepDefinitionRegistry,
    /// Client socket for sending notifications (e.g., diagnostics).
    client: Option<ClientSocket>,
}

impl std::fmt::Debug for ServerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerState")
            .field("client_capabilities", &self.client_capabilities)
            .field("workspace_info", &self.workspace_info)
            .field(
                "workspace_root",
                &self.workspace_root.as_ref().map(WorkspaceRoot::path),
            )
            .field("workspace_folders", &self.workspace_folders)
            .field(
                "workspace_initialization_id",
                &self.workspace_initialization_id,
            )
            .field(
                "workspace_preparation_pending",
                &self.workspace_preparation_pending,
            )
            .field("workspace_task", &self.workspace_task.is_running())
            .field(
                "deferred_document_saves",
                &self.deferred_document_saves.len(),
            )
            .field("initialised", &self.initialized)
            .field("config", &self.config)
            .field("feature_indices", &self.feature_indices)
            .field("rust_step_indices", &self.rust_step_indices)
            .field("step_registry", &self.step_registry)
            .field("client", &self.client.as_ref().map(|_| "<ClientSocket>"))
            .finish()
    }
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
    /// assert!(!state.is_initialised());
    /// ```
    #[must_use]
    pub fn new(config: ServerConfig) -> Self {
        Self {
            client_capabilities: None,
            workspace_info: None,
            workspace_root: None,
            workspace_folders: Vec::new(),
            workspace_initialization_id: 0,
            workspace_preparation_pending: false,
            workspace_task: WorkspaceTask::default(),
            deferred_document_saves: DeferredDocumentSaves::default(),
            initialized: false,
            config,
            feature_indices: HashMap::new(),
            rust_step_indices: HashMap::new(),
            step_registry: StepDefinitionRegistry::default(),
            client: None,
        }
    }

    /// Store the client socket for sending notifications.
    pub fn set_client(&mut self, client: ClientSocket) {
        self.client = Some(client);
    }

    /// Access the client socket for sending notifications.
    #[must_use]
    pub fn client(&self) -> Option<&ClientSocket> {
        self.client.as_ref()
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
    /// Start a workspace initialization transaction for `folders`.
    ///
    /// Clearing retained workspace state prevents a cancelled initialization
    /// from leaking its folders or capability into a later retry.
    pub(crate) fn begin_workspace_initialization(
        &mut self,
        folders: Vec<WorkspaceFolder>,
        workspace_preparation_pending: bool,
    ) -> u64 {
        self.workspace_task.abort();
        self.workspace_initialization_id = self.workspace_initialization_id.wrapping_add(1);
        self.workspace_folders = folders;
        self.workspace_info = None;
        self.workspace_root = None;
        self.workspace_preparation_pending = workspace_preparation_pending;
        self.deferred_document_saves.clear();
        self.workspace_initialization_id
    }

    /// Return whether `initialization_id` may install workspace state.
    #[must_use]
    pub(crate) fn is_current_workspace_initialization(&self, initialization_id: u64) -> bool {
        self.workspace_initialization_id == initialization_id
    }
    /// Return whether did-save work must wait for the workspace capability.
    #[must_use]
    pub(crate) fn workspace_preparation_pending(&self) -> bool {
        self.workspace_preparation_pending
    }
    /// Retain a did-save notification until workspace preparation completes.
    pub(crate) fn defer_document_save(
        &mut self,
        params: lsp_types::DidSaveTextDocumentParams,
    ) -> Result<usize, DeferredSaveDropReason> {
        self.deferred_document_saves.push(params)
    }
    /// Finish the current workspace preparation and return deferred saves.
    pub(crate) fn finish_workspace_initialization(
        &mut self,
        initialization_id: u64,
    ) -> Option<Vec<lsp_types::DidSaveTextDocumentParams>> {
        if !self.is_current_workspace_initialization(initialization_id) {
            return None;
        }
        self.workspace_preparation_pending = false;
        Some(self.deferred_document_saves.take())
    }

    /// Return the number of did-save notifications awaiting workspace readiness.
    #[must_use]
    pub(crate) fn deferred_document_save_count(&self) -> usize {
        self.deferred_document_saves.len()
    }

    /// Duplicate the workspace capability for a background indexing worker.
    pub(crate) fn workspace_root_for_replay(&self) -> Option<WorkspaceRoot> {
        self.workspace_root
            .as_ref()
            .and_then(|workspace_root| match workspace_root.try_clone() {
                Ok(workspace_root) => Some(workspace_root),
                Err(error) => {
                    warn!(error = %error, "failed to duplicate workspace-root capability");
                    None
                }
            })
    }

    /// Access the workspace folders provided by the client.
    #[must_use]
    pub fn workspace_folders(&self) -> &[WorkspaceFolder] {
        &self.workspace_folders
    }

    /// Store discovered workspace information and its read capability.
    ///
    /// # Errors
    ///
    /// Returns a [`ServerError`] when the workspace root is not UTF-8 or its
    /// capability-scoped directory cannot be opened.
    pub fn set_workspace_info(&mut self, workspace_info: WorkspaceInfo) -> Result<(), ServerError> {
        self.set_workspace_root(&workspace_info.root)?;
        self.workspace_info = Some(workspace_info);
        Ok(())
    }

    /// Store discovered workspace information together with a capability that
    /// was already opened off the LSP executor.
    ///
    /// Unlike [`Self::set_workspace_info`], this does not reopen the capability,
    /// so it is safe to call on the router task after the caller awaited the
    /// blocking open.
    pub(crate) fn install_workspace_info_with_root(
        &mut self,
        workspace_info: WorkspaceInfo,
        root: WorkspaceRoot,
    ) {
        self.workspace_root = Some(root);
        self.workspace_info = Some(workspace_info);
    }

    /// Store the capability-scoped root selected for workspace file reads.
    ///
    /// # Errors
    ///
    /// Returns a [`ServerError`] when the workspace root is not UTF-8 or its
    /// capability-scoped directory cannot be opened.
    pub(crate) fn set_workspace_root(&mut self, path: &Path) -> Result<(), ServerError> {
        self.workspace_root = Some(WorkspaceRoot::open(path)?);
        Ok(())
    }

    /// Install a workspace-root capability that was already opened off the LSP
    /// executor.
    ///
    /// This never touches the filesystem on the router task; the caller is
    /// responsible for having performed the blocking open beforehand.
    pub(crate) fn install_workspace_root(&mut self, root: WorkspaceRoot) {
        self.workspace_root = Some(root);
    }

    /// Access discovered workspace information, if available.
    #[must_use]
    pub fn workspace_info(&self) -> Option<&WorkspaceInfo> {
        self.workspace_info.as_ref()
    }

    /// Index a disk-backed feature file through the workspace-root capability.
    ///
    /// The workspace-relative path validation and capability-rooted read
    /// happen at this server boundary; the indexing domain only receives the
    /// owned source text via [`index_feature_source_owned`].
    pub(crate) fn index_feature_file(
        &self,
        path: &Path,
    ) -> Result<FeatureFileIndex, FeatureIndexError> {
        let workspace_root = self
            .workspace_root
            .as_ref()
            .ok_or(FeatureIndexError::WorkspaceRootUnavailable)?;
        let source = workspace_root.read_feature_source(path)?;
        index_feature_source_owned(path.to_path_buf(), source)
    }

    /// Access the current server configuration.
    #[must_use]
    pub fn config(&self) -> &ServerConfig {
        &self.config
    }

    /// Mark the server as initialized.
    pub fn mark_initialised(&mut self) {
        self.initialized = true;
    }

    /// Check if the server is initialized.
    #[must_use]
    pub fn is_initialised(&self) -> bool {
        self.initialized
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

    /// Iterate over all cached feature file indices.
    pub fn all_feature_indices(&self) -> impl Iterator<Item = &FeatureFileIndex> {
        self.feature_indices.values()
    }

    /// Retrieve the cached index for a Rust source file, if present.
    #[must_use]
    pub fn rust_step_index(&self, path: &Path) -> Option<&RustStepFileIndex> {
        self.rust_step_indices.get(path)
    }

    /// Access the compiled step registry.
    #[must_use]
    pub fn step_registry(&self) -> &StepDefinitionRegistry {
        &self.step_registry
    }

    /// Insert or update the cached index for a Rust source file.
    ///
    /// This also refreshes the compiled step registry entries for the file so
    /// navigation and diagnostics have keyword-keyed access to compiled
    /// patterns without a full reindex.
    pub fn upsert_rust_step_index(&mut self, index: RustStepFileIndex) {
        let path = index.path.clone();
        let compile_errors = self.step_registry.replace_rust_file(&index);
        self.rust_step_indices.insert(path.clone(), index);

        if !compile_errors.is_empty() {
            warn!(
                path = %path.display(),
                errors = compile_errors.len(),
                "failed to compile one or more step patterns"
            );
            for err in compile_errors {
                warn!(path = %path.display(), error = %err, "step pattern compilation error");
            }
        }
    }
}

/// Build the server capabilities to advertise to the client.
///
/// Phase 7 advertises text document sync to receive save notifications for
/// `.feature` file indexing, definition navigation for Rust-to-feature step
/// navigation, and implementation navigation for feature-to-Rust step
/// navigation.
#[must_use]
pub fn build_server_capabilities() -> ServerCapabilities {
    ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Options(
            TextDocumentSyncOptions {
                open_close: Some(true),
                change: Some(TextDocumentSyncKind::INCREMENTAL),
                save: Some(lsp_types::TextDocumentSyncSaveOptions::SaveOptions(
                    lsp_types::SaveOptions {
                        include_text: Some(true),
                    },
                )),
                ..Default::default()
            },
        )),
        definition_provider: Some(lsp_types::OneOf::Left(true)),
        implementation_provider: Some(lsp_types::ImplementationProviderCapability::Simple(true)),
        ..ServerCapabilities::default()
    }
}

#[cfg(test)]
mod tests;
