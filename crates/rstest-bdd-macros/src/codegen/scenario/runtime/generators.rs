//! Individual code generators for runtime scaffolding components.
//!
//! This module contains generators for the runtime code that executes BDD scenarios.
//! The generators are split into submodules by concern:
//!
//! - [`step`]: Step execution generators (executor, decoder, loop)
//! - [`scenario`]: Scenario-level generators (guard, skip handler)

mod scenario;
mod step;

pub(super) use scenario::{generate_scenario_guard, generate_skip_handler};
pub(super) use step::{generate_skip_decoder, generate_step_executor, generate_step_executor_loop};
