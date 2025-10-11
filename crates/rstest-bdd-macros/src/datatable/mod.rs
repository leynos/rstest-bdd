//! Derive entry points for typed data table helpers.
//!
//! This module wires the outer derive macro surfaces to the internal code
//! generators that live in the sibling modules.

mod config;
mod parser;
mod rename;
mod row;
mod table;
mod validation;

use proc_macro::TokenStream;

pub(crate) fn derive_data_table_row(input: TokenStream) -> TokenStream {
    row::expand(input)
}

pub(crate) fn derive_data_table(input: TokenStream) -> TokenStream {
    table::expand(input)
}
