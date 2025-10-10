mod rename;
mod row;
mod table;

use proc_macro::TokenStream;

pub(crate) fn derive_data_table_row(input: TokenStream) -> TokenStream {
    row::expand(input)
}

pub(crate) fn derive_data_table(input: TokenStream) -> TokenStream {
    table::expand(input)
}
