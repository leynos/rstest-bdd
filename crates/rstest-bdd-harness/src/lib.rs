//! Harness adapter contracts for `rstest-bdd`.
//!
//! This crate provides a framework-agnostic interface for executing scenario
//! runners and supplying test attributes through policy plug-ins.

mod adapter;
#[doc(hidden)]
pub mod binary_test_support;
mod error;
#[doc(hidden)]
pub mod macrotest_support;
mod policy;
pub mod policy_conformance;
mod runner;
mod std_harness;
#[cfg(test)]
mod test_utils;
#[cfg(feature = "testing")]
mod testing;
#[doc(hidden)]
pub mod trybuild_staging;

pub use adapter::HarnessAdapter;
pub use error::{HarnessError, HarnessResult};
pub use policy::{AttributePolicy, DefaultAttributePolicy, TestAttribute};
pub use runner::{
    ScenarioMetadata, ScenarioRunRequest, ScenarioRunner, StdScenarioRunRequest, StdScenarioRunner,
};
pub use std_harness::StdHarness;
#[cfg(feature = "testing")]
pub use testing::FailingHarness;
pub use tracing;
