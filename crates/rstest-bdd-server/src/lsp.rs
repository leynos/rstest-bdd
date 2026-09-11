//! The single LSP protocol type facade for this server.
//!
//! Protocol types are re-exported here from `async_lsp::lsp_types`, so the
//! whole crate speaks one LSP model type family. Route every router handler
//! parameter, `ClientSocket` payload, server-state field, diagnostic, request,
//! response, and URI through this module.
//!
//! `async-lsp` owns the `lsp-types` version: its router and client-socket
//! signatures are written in those types. Do not add a direct `lsp-types`
//! dependency unless the transport dependency resolves to the same version,
//! because two copies of the model types cannot meet across that boundary.
//! Upgrading `lsp-types` therefore requires an `async-lsp` release whose
//! dependency range covers the target version; move both together.

pub use async_lsp::lsp_types::*;
