//! Semantic error types for the language server.
//!
//! This module defines error types that provide meaningful context about
//! failures during language server operations. Errors are designed to be
//! inspectable by callers for appropriate handling.

use thiserror::Error;

/// Errors that can occur during language server operations.
///
/// Each variant provides specific context about the failure, enabling
/// appropriate error handling and user-facing messages.
#[derive(Debug, Error)]
pub enum ServerError {
    /// Failed to discover workspace root from the given path.
    #[error("workspace discovery failed: {0}")]
    WorkspaceDiscovery(String),

    /// The cargo metadata command failed.
    #[error("cargo metadata failed: {0}")]
    CargoMetadata(#[from] cargo_metadata::Error),

    /// Server received a request before initialisation completed.
    #[error("server not initialised")]
    NotInitialised,

    /// Server received a duplicate initialisation request.
    #[error("server already initialised")]
    AlreadyInitialised,

    /// An invalid configuration value was provided.
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),

    /// An I/O error occurred.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_discovery_error_displays_message() {
        let error = ServerError::WorkspaceDiscovery("no Cargo.toml found".to_string());
        assert_eq!(
            error.to_string(),
            "workspace discovery failed: no Cargo.toml found"
        );
    }

    #[test]
    fn not_initialised_error_displays_message() {
        let error = ServerError::NotInitialised;
        assert_eq!(error.to_string(), "server not initialised");
    }

    #[test]
    fn already_initialised_error_displays_message() {
        let error = ServerError::AlreadyInitialised;
        assert_eq!(error.to_string(), "server already initialised");
    }

    #[test]
    fn invalid_config_error_displays_message() {
        let error = ServerError::InvalidConfig("unknown log level".to_string());
        assert_eq!(
            error.to_string(),
            "invalid configuration: unknown log level"
        );
    }

    #[test]
    fn io_error_converts_from_std_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let error: ServerError = io_err.into();
        assert!(error.to_string().contains("file not found"));
    }
}
