//! Wrapper function generation for step definitions.

pub(crate) mod args;
pub(crate) mod emit;

#[expect(
    unused_imports,
    reason = "re-exports preserve public API compatibility"
)]
pub(crate) use args::{
    CallArg, DataTableArg, DocStringArg, ExtractedArgs, FixtureArg, StepArg, extract_args,
};
pub(crate) use emit::{WrapperConfig, generate_wrapper_code};
