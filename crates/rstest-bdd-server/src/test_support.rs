//! Shared test support utilities for rstest-bdd-server tests.
//!
//! This module provides common infrastructure for both unit and integration
//! tests, including:
//! - Temporary directory and file management
//! - File indexing via simulated LSP save events
//! - Scenario building for diagnostic and navigation tests

use lsp_types::{DidSaveTextDocumentParams, TextDocumentIdentifier, Url};
use std::path::Path;
use tempfile::TempDir;

use crate::config::ServerConfig;
use crate::handlers::handle_did_save_text_document;
use crate::server::ServerState;

/// Index a file by simulating an LSP `textDocument/didSave` event.
///
/// This triggers the server's file indexing logic for the given path,
/// populating the feature index or step registry as appropriate.
///
/// # Panics
///
/// Panics if the file path cannot be converted to a valid URI.
#[expect(clippy::expect_used, reason = "test helper uses expect for clarity")]
pub fn index_file(state: &mut ServerState, path: &Path) {
    let uri = Url::from_file_path(path).expect("file URI");
    let params = DidSaveTextDocumentParams {
        text_document: TextDocumentIdentifier { uri },
        text: None,
    };
    handle_did_save_text_document(state, params);
}

/// Builder for constructing test scenarios with multiple feature and Rust files.
///
/// Provides a fluent API for adding files and building a scenario with
/// all files written and indexed.
pub struct ScenarioBuilder {
    dir: TempDir,
    feature_files: Vec<(String, String)>,
    rust_files: Vec<(String, String)>,
    state: ServerState,
}

impl ScenarioBuilder {
    /// Create a new scenario builder with a fresh temp directory and server state.
    ///
    /// # Panics
    ///
    /// Panics if the temporary directory cannot be created.
    #[expect(clippy::expect_used, reason = "builder panics on temp dir failure")]
    #[must_use]
    pub fn new() -> Self {
        let dir = TempDir::new().expect("temp dir");
        Self {
            dir,
            feature_files: Vec::new(),
            rust_files: Vec::new(),
            state: ServerState::new(ServerConfig::default()),
        }
    }

    /// Add a feature file to be created and indexed.
    #[must_use]
    pub fn with_feature(mut self, filename: impl Into<String>, content: impl Into<String>) -> Self {
        self.feature_files.push((filename.into(), content.into()));
        self
    }

    /// Add a Rust step definition file to be created and indexed.
    #[must_use]
    pub fn with_rust_steps(
        mut self,
        filename: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        self.rust_files.push((filename.into(), content.into()));
        self
    }

    /// Build the scenario, writing and indexing all files.
    ///
    /// Returns the temp directory (for path construction) and the server state
    /// (for querying indices and computing diagnostics).
    ///
    /// # Panics
    ///
    /// Panics if any file cannot be written.
    #[expect(clippy::expect_used, reason = "builder panics on write failure")]
    #[must_use]
    pub fn build(mut self) -> (TempDir, ServerState) {
        // Write and index feature files first
        for (filename, content) in &self.feature_files {
            let path = self.dir.path().join(filename);
            std::fs::write(&path, content).expect("write feature file");
            index_file(&mut self.state, &path);
        }
        // Write and index Rust files
        for (filename, content) in &self.rust_files {
            let path = self.dir.path().join(filename);
            std::fs::write(&path, content).expect("write rust file");
            index_file(&mut self.state, &path);
        }
        (self.dir, self.state)
    }
}

impl Default for ScenarioBuilder {
    fn default() -> Self {
        Self::new()
    }
}
