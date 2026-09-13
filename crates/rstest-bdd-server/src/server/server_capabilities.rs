//! LSP server capabilities advertised during initialization.

use crate::lsp::{
    ServerCapabilities,
    TextDocumentSyncCapability,
    TextDocumentSyncKind,
    TextDocumentSyncOptions,
};

/// Build the server capabilities to advertise to the client.
///
/// Advertises text document sync to receive save notifications for `.feature`
/// file indexing, definition navigation for Rust-to-feature step navigation,
/// and implementation navigation for feature-to-Rust step navigation.
#[must_use]
pub fn build_server_capabilities() -> ServerCapabilities {
    ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Options(
            TextDocumentSyncOptions {
                open_close: Some(true),
                change: Some(TextDocumentSyncKind::INCREMENTAL),
                save: Some(crate::lsp::TextDocumentSyncSaveOptions::SaveOptions(
                    crate::lsp::SaveOptions {
                        include_text: Some(true),
                    },
                )),
                ..Default::default()
            },
        )),
        definition_provider: Some(crate::lsp::OneOf::Left(true)),
        implementation_provider: Some(crate::lsp::ImplementationProviderCapability::Simple(true)),
        ..ServerCapabilities::default()
    }
}
