//! Helper functions for diagnostic integration tests.
//!
//! These helpers are organized into submodules by diagnostic type, so each
//! test binary imports only from the submodule it needs—avoiding dead code
//! warnings without file-level lint suppression.

pub mod basic;
pub mod placeholder;
pub mod table_docstring;
