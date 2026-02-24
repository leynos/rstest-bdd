//! Harness adapter contracts for `rstest-bdd`.
//!
//! This crate provides a framework-agnostic interface for executing scenario
//! runners and supplying test attributes through policy plug-ins.

mod adapter;
mod policy;
mod runner;
mod std_harness;

pub use adapter::HarnessAdapter;
pub use policy::{AttributePolicy, DefaultAttributePolicy, TestAttribute};
pub use runner::{ScenarioMetadata, ScenarioRunRequest, ScenarioRunner};
pub use std_harness::{STD_HARNESS_PANIC_MESSAGE, StdHarness};
