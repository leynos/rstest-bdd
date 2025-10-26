//! Wrapper function generation for step definitions.

pub mod args;
mod arguments;
pub(crate) mod emit;

#[expect(unused_imports, reason = "re-exports expose helper API")]
pub use args::{extract_args, CallArg};
pub(crate) use emit::{generate_wrapper_code, WrapperConfig};
