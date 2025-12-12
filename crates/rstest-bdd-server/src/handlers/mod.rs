//! LSP request and notification handlers.
//!
//! This module contains handlers for various LSP protocol messages. Currently
//! implements lifecycle handlers (initialise/shutdown); navigation and
//! diagnostic handlers will be added in future phases.

mod lifecycle;

pub use lifecycle::{handle_initialise, handle_initialised, handle_shutdown};
