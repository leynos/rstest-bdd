//! Workspace discovery and file scanning.
//!
//! This module provides functionality for discovering Rust workspaces and
//! locating relevant files for the language server.

mod workspace;

pub use workspace::{discover_workspace, find_feature_files, WorkspaceInfo};
