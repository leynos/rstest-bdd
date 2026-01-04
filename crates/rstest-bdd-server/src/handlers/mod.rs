//! LSP request and notification handlers.
//!
//! This module contains handlers for various LSP protocol messages. Currently
//! implements lifecycle handlers, on-save indexing, definition navigation,
//! implementation navigation, and diagnostic publishing.

mod definition;
mod diagnostics;
mod implementation;
mod lifecycle;
mod text_document;
pub mod util;

pub use definition::handle_definition;
pub use diagnostics::{
    compute_unimplemented_step_diagnostics, compute_unused_step_diagnostics,
    publish_all_feature_diagnostics, publish_feature_diagnostics, publish_rust_diagnostics,
};
pub use implementation::handle_implementation;
pub use lifecycle::{handle_initialise, handle_initialised, handle_shutdown};
pub use text_document::handle_did_save_text_document;
