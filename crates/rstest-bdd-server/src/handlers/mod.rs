//! LSP request and notification handlers.
//!
//! This module contains handlers for various LSP protocol messages. Currently
//! implements lifecycle handlers and an on-save indexing handler; navigation
//! and diagnostic handlers will be added in future phases.

mod lifecycle;
mod text_document;

pub use lifecycle::{handle_initialise, handle_initialised, handle_shutdown};
pub use text_document::handle_did_save_text_document;
