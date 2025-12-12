//! Wrapper function generation for step definitions.

pub mod args;
mod arguments;
mod datatable_shared;
pub(crate) mod emit;

pub use args::extract_args;
pub(crate) use emit::{WrapperConfig, generate_wrapper_code};
