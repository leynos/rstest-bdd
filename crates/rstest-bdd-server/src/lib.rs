//! Language Server Protocol implementation for rstest-bdd.
//!
//! This crate provides an LSP server for the rstest-bdd BDD testing framework,
//! enabling IDE integration for navigation between Rust step definitions and
//! Gherkin feature files.
//!
//! # Overview
//!
//! The server communicates via JSON-RPC over stdin/stdout and supports:
//!
//! - Workspace discovery via `cargo metadata`
//! - LSP lifecycle management (initialise/shutdown)
//! - Structured logging with environment variable configuration
//!
//! # Configuration
//!
//! The server can be configured via environment variables:
//!
//! - `RSTEST_BDD_LSP_LOG_LEVEL`: Log verbosity (trace, debug, info, warn,
//!   error)
//! - `RSTEST_BDD_LSP_DEBOUNCE_MS`: Delay before processing file changes
//!
//! # Example
//!
//! ```ignore
//! use rstest_bdd_server::config::ServerConfig;
//! use rstest_bdd_server::server::ServerState;
//!
//! let config = ServerConfig::from_env()?;
//! let state = ServerState::new(config);
//! ```

pub mod config;
pub mod discovery;
pub mod error;
pub mod handlers;
pub mod indexing;
pub mod logging;
pub mod server;

/// Test support utilities for unit and integration tests.
///
/// This module is hidden from documentation as it's intended for internal
/// test use only.
#[doc(hidden)]
pub mod test_support;
