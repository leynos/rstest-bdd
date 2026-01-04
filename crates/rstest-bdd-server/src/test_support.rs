//! Shared test support utilities for rstest-bdd-server tests.
//!
//! This module provides common infrastructure for both unit and integration
//! tests, including:
//! - Temporary directory and file management
//! - File indexing via simulated LSP save events
//! - Scenario building for diagnostic and navigation tests
//! - Newtype wrappers for improved type safety

use lsp_types::{DidSaveTextDocumentParams, TextDocumentIdentifier, Url};
use std::path::Path;
use tempfile::TempDir;

use crate::config::ServerConfig;
use crate::handlers::handle_did_save_text_document;
use crate::server::ServerState;

/// Newtype wrapper for test file names to improve type safety.
#[derive(Debug, Clone)]
pub struct Filename(pub(crate) String);

impl From<&str> for Filename {
    fn from(s: &str) -> Self {
        Self(s.into())
    }
}

impl From<String> for Filename {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl AsRef<str> for Filename {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Newtype wrapper for file contents to improve type safety.
#[derive(Debug, Clone)]
pub struct FileContent(pub(crate) String);

impl From<&str> for FileContent {
    fn from(s: &str) -> Self {
        Self(s.into())
    }
}

impl From<String> for FileContent {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl AsRef<str> for FileContent {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Specifies which diagnostic checks to run in parameterised tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticCheckType {
    /// Check only Rust file diagnostics.
    Rust,
    /// Check only feature file diagnostics.
    Feature,
    /// Check both Rust and feature file diagnostics.
    Both,
}

/// Result of building a test scenario.
///
/// Contains the temporary directory (for constructing file paths) and the
/// server state (for querying indices and computing diagnostics).
pub struct TestScenario {
    /// Temporary directory containing the test files.
    pub dir: TempDir,
    /// Server state with indexed files.
    pub state: ServerState,
}

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

    /// Helper to add a file to a specific collection.
    fn add_file(
        collection: &mut Vec<(String, String)>,
        filename: impl Into<Filename>,
        content: impl Into<FileContent>,
    ) {
        let filename = filename.into();
        let content = content.into();
        collection.push((filename.0, content.0));
    }

    /// Add a feature file to be created and indexed.
    #[must_use]
    pub fn with_feature(
        mut self,
        filename: impl Into<Filename>,
        content: impl Into<FileContent>,
    ) -> Self {
        Self::add_file(&mut self.feature_files, filename, content);
        self
    }

    /// Add a Rust step definition file to be created and indexed.
    #[must_use]
    pub fn with_rust_steps(
        mut self,
        filename: impl Into<Filename>,
        content: impl Into<FileContent>,
    ) -> Self {
        Self::add_file(&mut self.rust_files, filename, content);
        self
    }

    /// Build the scenario, writing and indexing all files.
    ///
    /// Returns a [`TestScenario`] containing the temp directory (for path
    /// construction) and the server state (for querying indices and computing
    /// diagnostics).
    ///
    /// # Panics
    ///
    /// Panics if any file cannot be written.
    #[expect(clippy::expect_used, reason = "builder panics on write failure")]
    #[must_use]
    pub fn build(mut self) -> TestScenario {
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
        TestScenario {
            dir: self.dir,
            state: self.state,
        }
    }
}

impl Default for ScenarioBuilder {
    fn default() -> Self {
        Self::new()
    }
}
