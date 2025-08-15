//! Wrapper function generation for step definitions.

pub mod args;
pub(crate) mod emit;

#[expect(unused_imports, reason = "re-exports expose helper API")]
pub use args::{CallArg, extract_args};
pub(crate) use emit::{WrapperConfig, generate_wrapper_code};
