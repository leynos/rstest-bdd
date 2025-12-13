//! Text document notification handlers.
//!
//! Phase 7 focuses on building language-server foundations. The first feature
//! delivered is an on-save indexing pipeline for `.feature` files. Indexing is
//! performed via the `gherkin` parser and stored in the shared server state.

use lsp_types::DidSaveTextDocumentParams;
use tracing::{debug, warn};

use crate::indexing::index_feature_file;
use crate::server::ServerState;

/// Handle `textDocument/didSave` notifications.
///
/// When a saved document is a `.feature` file, the server parses and indexes
/// it using the Gherkin parser. Parse failures are logged and do not surface
/// as diagnostics yet (that is handled in later roadmap phases).
pub fn handle_did_save_text_document(state: &mut ServerState, params: DidSaveTextDocumentParams) {
    let uri = params.text_document.uri;
    let Ok(path) = uri.to_file_path() else {
        debug!(%uri, "ignoring didSave for non-file URI");
        return;
    };

    if !matches!(path.extension(), Some(ext) if ext == "feature") {
        return;
    }

    match index_feature_file(&path) {
        Ok(index) => {
            debug!(
                path = %path.display(),
                steps = index.steps.len(),
                examples = index.example_columns.len(),
                "indexed feature file"
            );
            state.upsert_feature_index(index);
        }
        Err(err) => {
            warn!(path = %path.display(), error = %err, "failed to index feature file");
        }
    }
}
