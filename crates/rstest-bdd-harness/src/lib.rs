//! Harness adapter contracts for `rstest-bdd`.
//!
//! This crate provides a framework-agnostic interface for executing scenario
//! runners and supplying test attributes through policy plug-ins.

mod adapter;
mod error;
mod policy;
mod runner;
mod std_harness;
#[cfg(test)]
pub(crate) mod test_utils;
#[doc(hidden)]
pub mod trybuild_staging;

pub use adapter::HarnessAdapter;
pub use error::{HarnessError, HarnessResult};
pub use policy::{AttributePolicy, DefaultAttributePolicy, TestAttribute};
pub use runner::{
    ScenarioMetadata, ScenarioRunRequest, ScenarioRunner, StdScenarioRunRequest, StdScenarioRunner,
};
pub use std_harness::StdHarness;
pub use tracing;
