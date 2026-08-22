//! Bounded, coalescing storage for did-save work awaiting workspace readiness.
//!
//! This private helper is owned solely by [`super::ServerState`]. It preserves
//! each document's latest save in order while bounding retained source text.

use std::collections::VecDeque;

use lsp_types::DidSaveTextDocumentParams;

const MAX_DEFERRED_DOCUMENT_SAVES: usize = 128;
const MAX_DEFERRED_DOCUMENT_BYTES: usize = 4 * 1024 * 1024;

/// Why a deferred document save could not be retained.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DeferredSaveDropReason {
    /// Retaining another document would exceed the fixed document limit.
    DocumentLimit,
    /// Retaining source text would exceed the fixed byte limit.
    ByteLimit,
}

impl DeferredSaveDropReason {
    /// Return the fixed metric outcome for this bounded-queue rejection.
    pub(crate) fn metric_outcome(self) -> &'static str {
        match self {
            Self::DocumentLimit => "document-limit",
            Self::ByteLimit => "byte-limit",
        }
    }
}

/// Bounded deferred did-save notifications.
#[derive(Debug, Default)]
pub(super) struct DeferredDocumentSaves {
    saves: VecDeque<DidSaveTextDocumentParams>,
    byte_count: usize,
}

impl DeferredDocumentSaves {
    /// Coalesce a save by URI and retain it if the fixed limits permit it.
    pub(super) fn push(
        &mut self,
        params: DidSaveTextDocumentParams,
    ) -> Result<usize, DeferredSaveDropReason> {
        let byte_count = save_byte_count(&params);
        let existing_index = self
            .saves
            .iter()
            .position(|save| save.text_document.uri == params.text_document.uri);
        let existing_byte_count = existing_index
            .and_then(|index| self.saves.get(index))
            .map_or(0, save_byte_count);
        let retained_count = self.saves.len() - usize::from(existing_index.is_some());
        let retained_bytes = self.byte_count - existing_byte_count;

        if retained_count >= MAX_DEFERRED_DOCUMENT_SAVES {
            return Err(DeferredSaveDropReason::DocumentLimit);
        }
        if retained_bytes.saturating_add(byte_count) > MAX_DEFERRED_DOCUMENT_BYTES {
            return Err(DeferredSaveDropReason::ByteLimit);
        }

        if let Some(index) = existing_index
            && let Some(previous) = self.saves.remove(index)
        {
            self.byte_count -= save_byte_count(&previous);
        }
        self.byte_count += byte_count;
        self.saves.push_back(params);
        Ok(self.saves.len())
    }

    /// Remove every deferred save and return them in retained order.
    pub(super) fn take(&mut self) -> Vec<DidSaveTextDocumentParams> {
        self.byte_count = 0;
        self.saves.drain(..).collect()
    }

    /// Discard all deferred saves when a later initialization supersedes them.
    pub(super) fn clear(&mut self) {
        self.byte_count = 0;
        self.saves.clear();
    }

    /// Return the number of retained saves.
    pub(super) fn len(&self) -> usize {
        self.saves.len()
    }
}

fn save_byte_count(params: &DidSaveTextDocumentParams) -> usize {
    params.text_document.uri.as_str().len() + params.text.as_ref().map_or(0, String::len)
}

#[cfg(test)]
mod tests {
    //! Tests for bounded, coalescing deferred did-save storage.

    use lsp_types::{TextDocumentIdentifier, Url};

    use super::*;

    fn save(uri: &str, text: &str) -> DidSaveTextDocumentParams {
        let Ok(uri) = Url::parse(uri) else {
            panic!("test URI must parse");
        };
        DidSaveTextDocumentParams {
            text_document: TextDocumentIdentifier { uri },
            text: Some(text.to_owned()),
        }
    }

    #[test]
    fn coalesces_newer_saves_and_preserves_latest_order() {
        let mut saves = DeferredDocumentSaves::default();

        assert_eq!(saves.push(save("file:///a.rs", "first")), Ok(1));
        assert_eq!(saves.push(save("file:///b.rs", "second")), Ok(2));
        assert_eq!(saves.push(save("file:///a.rs", "third")), Ok(2));

        let replay = saves.take();
        let [first, second] = replay.as_slice() else {
            panic!("expected two coalesced saves");
        };
        assert_eq!(first.text_document.uri.as_str(), "file:///b.rs");
        assert_eq!(second.text.as_deref(), Some("third"));
    }

    #[test]
    fn rejects_a_save_larger_than_the_fixed_byte_limit() {
        let mut saves = DeferredDocumentSaves::default();
        let source = "x".repeat(MAX_DEFERRED_DOCUMENT_BYTES);

        assert_eq!(
            saves.push(save("file:///large.rs", &source)),
            Err(DeferredSaveDropReason::ByteLimit)
        );
        assert_eq!(saves.len(), 0);
    }

    #[test]
    fn rejects_a_new_document_after_the_fixed_document_limit() {
        let mut saves = DeferredDocumentSaves::default();

        for index in 0..MAX_DEFERRED_DOCUMENT_SAVES {
            let uri = format!("file:///deferred-{index}.rs");
            assert!(saves.push(save(&uri, "source")).is_ok());
        }

        assert_eq!(
            saves.push(save("file:///deferred-overflow.rs", "source")),
            Err(DeferredSaveDropReason::DocumentLimit)
        );
        assert_eq!(saves.len(), MAX_DEFERRED_DOCUMENT_SAVES);
    }
}
