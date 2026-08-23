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
        self.push_with_limits(
            params,
            MAX_DEFERRED_DOCUMENT_SAVES,
            MAX_DEFERRED_DOCUMENT_BYTES,
        )
    }

    fn push_with_limits(
        &mut self,
        params: DidSaveTextDocumentParams,
        maximum_documents: usize,
        maximum_bytes: usize,
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

        if retained_count >= maximum_documents {
            return Err(DeferredSaveDropReason::DocumentLimit);
        }
        if retained_bytes.saturating_add(byte_count) > maximum_bytes {
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

    #[cfg(test)]
    fn recomputed_byte_count(&self) -> usize {
        self.saves.iter().map(save_byte_count).sum()
    }
}

fn save_byte_count(params: &DidSaveTextDocumentParams) -> usize {
    params.text_document.uri.as_str().len() + params.text.as_ref().map_or(0, String::len)
}

#[cfg(test)]
mod tests {
    //! Tests for bounded, coalescing deferred did-save storage.

    use lsp_types::{TextDocumentIdentifier, Url};
    use proptest::prelude::*;

    use super::*;

    const DEFERRED_SAVE_URI_ALPHABET: usize = 130;
    const TEST_MAXIMUM_DOCUMENTS: usize = 8;
    const TEST_MAXIMUM_DOCUMENT_URI_ALPHABET: usize = 8;
    const TEST_MAXIMUM_BYTES: usize = 160;

    fn save(uri: &str, text: &str) -> DidSaveTextDocumentParams {
        let Ok(uri) = Url::parse(uri) else {
            panic!("test URI must parse");
        };
        DidSaveTextDocumentParams {
            text_document: TextDocumentIdentifier { uri },
            text: Some(text.to_owned()),
        }
    }

    #[derive(Clone, Debug)]
    enum DeferredSaveOperation {
        Push { uri: usize, source_length: usize },
        Take,
        Clear,
    }

    fn deferred_save_operations() -> impl Strategy<Value = Vec<DeferredSaveOperation>> {
        let random_operations = prop::collection::vec(
            prop_oneof![
                (0_usize..DEFERRED_SAVE_URI_ALPHABET, 20_usize..48).prop_map(
                    |(uri, source_length)| { DeferredSaveOperation::Push { uri, source_length } }
                ),
                Just(DeferredSaveOperation::Take),
                Just(DeferredSaveOperation::Clear),
            ],
            0..160,
        );
        (
            prop::collection::vec(20_usize..48, MAX_DEFERRED_DOCUMENT_SAVES + 1..=144),
            random_operations,
        )
            .prop_map(|(source_lengths, mut operations)| {
                let mut forced_rejection = source_lengths
                    .into_iter()
                    .enumerate()
                    .map(|(index, source_length)| DeferredSaveOperation::Push {
                        uri: index,
                        source_length,
                    })
                    .collect::<Vec<_>>();
                forced_rejection.append(&mut operations);
                forced_rejection
            })
    }

    fn save_for_uri(uri: usize, source_length: usize) -> DidSaveTextDocumentParams {
        save(
            &format!("file:///deferred-{uri}.rs"),
            &"x".repeat(source_length),
        )
    }

    fn retain_latest_save(
        expected: &mut Vec<DidSaveTextDocumentParams>,
        save: DidSaveTextDocumentParams,
    ) {
        if let Some(index) = expected
            .iter()
            .position(|expected_save| expected_save.text_document.uri == save.text_document.uri)
        {
            expected.remove(index);
        }
        expected.push(save);
    }

    fn assert_deferred_save_invariants(saves: &DeferredDocumentSaves) {
        assert_eq!(saves.byte_count, saves.recomputed_byte_count());
        assert!(saves.len() <= MAX_DEFERRED_DOCUMENT_SAVES);
        assert!(saves.byte_count <= MAX_DEFERRED_DOCUMENT_BYTES);
    }

    fn assert_rejected_push_preserves_state(
        saves: &DeferredDocumentSaves,
        previous_saves: &VecDeque<DidSaveTextDocumentParams>,
        previous_byte_count: usize,
    ) {
        assert_eq!(&saves.saves, previous_saves);
        assert_eq!(saves.byte_count, previous_byte_count);
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

        let previous_saves = saves.saves.clone();
        let previous_byte_count = saves.byte_count;
        assert_eq!(
            saves.push(save("file:///deferred-overflow.rs", "source")),
            Err(DeferredSaveDropReason::DocumentLimit)
        );
        assert_eq!(saves.len(), MAX_DEFERRED_DOCUMENT_SAVES);
        assert_rejected_push_preserves_state(&saves, &previous_saves, previous_byte_count);
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(16))]

        #[test]
        fn preserves_deferred_save_invariants_across_operations(
            operations in deferred_save_operations()
        ) {
            let mut saves = DeferredDocumentSaves::default();
            let mut expected = Vec::new();
            let mut saw_document_limit = false;

            for operation in operations {
                match operation {
                    DeferredSaveOperation::Push { uri, source_length } => {
                        let save = save_for_uri(uri, source_length);
                        let previous_saves = saves.saves.clone();
                        let previous_byte_count = saves.byte_count;
                        match saves.push(save.clone()) {
                            Ok(_) => retain_latest_save(&mut expected, save),
                            Err(reason) => {
                                saw_document_limit |= reason == DeferredSaveDropReason::DocumentLimit;
                                assert_rejected_push_preserves_state(
                                    &saves,
                                    &previous_saves,
                                    previous_byte_count,
                                );
                            }
                        }
                    }
                    DeferredSaveOperation::Take => {
                        prop_assert_eq!(saves.take(), expected);
                        expected = Vec::new();
                    }
                    DeferredSaveOperation::Clear => {
                        saves.clear();
                        expected = Vec::new();
                    }
                }

                assert_deferred_save_invariants(&saves);
                prop_assert_eq!(
                    saves.saves.iter().collect::<Vec<_>>(),
                    expected.iter().collect::<Vec<_>>()
                );
            }
            prop_assert!(saw_document_limit);
        }

        #[test]
        fn preserves_rejection_state_for_bounded_cumulative_bytes(
            source_lengths in prop::collection::vec(20_usize..48, 4..80),
            random_operations in prop::collection::vec(
                prop_oneof![
                    (0_usize..TEST_MAXIMUM_DOCUMENT_URI_ALPHABET, 20_usize..48)
                        .prop_map(|(uri, source_length)| {
                            DeferredSaveOperation::Push { uri, source_length }
                        }),
                    Just(DeferredSaveOperation::Take),
                    Just(DeferredSaveOperation::Clear),
                ],
                0..80,
            ),
        ) {
            let mut saves = DeferredDocumentSaves::default();
            let mut expected = Vec::new();
            let mut saw_byte_limit = false;
            let mut operations = source_lengths
                .into_iter()
                .enumerate()
                .map(|(index, source_length)| DeferredSaveOperation::Push {
                    uri: index,
                    source_length,
                })
                .collect::<Vec<_>>();
            operations.extend(random_operations);

            for operation in operations {
                match operation {
                    DeferredSaveOperation::Push { uri, source_length } => {
                        let save = save_for_uri(uri, source_length);
                        let previous_saves = saves.saves.clone();
                        let previous_byte_count = saves.byte_count;
                        match saves.push_with_limits(
                            save.clone(),
                            TEST_MAXIMUM_DOCUMENTS,
                            TEST_MAXIMUM_BYTES,
                        ) {
                            Ok(_) => retain_latest_save(&mut expected, save),
                            Err(reason) => {
                                saw_byte_limit |= reason == DeferredSaveDropReason::ByteLimit;
                                assert_rejected_push_preserves_state(
                                    &saves,
                                    &previous_saves,
                                    previous_byte_count,
                                );
                            }
                        }
                    }
                    DeferredSaveOperation::Take => {
                        prop_assert_eq!(saves.take(), expected);
                        expected = Vec::new();
                    }
                    DeferredSaveOperation::Clear => {
                        saves.clear();
                        expected = Vec::new();
                    }
                }
                assert_deferred_save_invariants(&saves);
                prop_assert_eq!(
                    saves.saves.iter().collect::<Vec<_>>(),
                    expected.iter().collect::<Vec<_>>()
                );
            }
            prop_assert!(saw_byte_limit);
        }
    }
}
