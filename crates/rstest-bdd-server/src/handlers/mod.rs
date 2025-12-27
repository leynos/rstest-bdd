//! LSP request and notification handlers.
//!
//! This module contains handlers for various LSP protocol messages. Currently
//! implements lifecycle handlers, on-save indexing, definition navigation, and
//! implementation navigation; diagnostic handlers will be added in future
//! phases.

mod definition;
mod implementation;
mod lifecycle;
mod text_document;
pub mod util;

pub use definition::handle_definition;
pub use implementation::handle_implementation;
pub use lifecycle::{handle_initialise, handle_initialised, handle_shutdown};
pub use text_document::handle_did_save_text_document;
