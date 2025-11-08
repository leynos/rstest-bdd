//! Wrapper function generation for step definitions.

pub mod args;
mod arguments;
pub(crate) mod emit;

pub use args::extract_args;
pub(crate) use emit::{generate_wrapper_code, WrapperConfig};
