//! Individual code generators for runtime scaffolding components.
//!
//! This module contains generators for the runtime code that executes BDD scenarios.
//! The generators are split into submodules by concern:
//!
//! - [`step`]: Step execution generators (executor, decoder, loop, outline loop)
//! - [`scenario`]: Scenario-level generators (guard, skip handler)

mod outline;
mod scenario;
mod step;
mod step_loop;

pub(super) use outline::{
    generate_async_step_executor_loop_outline, generate_step_executor_loop_outline,
};
pub(super) use scenario::{generate_scenario_guard, generate_skip_handler};
pub(super) use step::{
    generate_async_step_executor, generate_skip_decoder, generate_step_executor,
};
pub(super) use step_loop::{generate_async_step_executor_loop, generate_step_executor_loop};
