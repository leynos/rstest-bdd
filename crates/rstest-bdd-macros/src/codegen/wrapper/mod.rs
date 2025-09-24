//! Wrapper function generation for step definitions.

mod arg_processing;
pub mod args;
mod config;
pub(crate) mod emit;

#[expect(unused_imports, reason = "re-exports expose helper API")]
pub use args::{CallArg, extract_args};
pub(crate) use config::WrapperConfig;
pub(crate) use emit::generate_wrapper_code;
