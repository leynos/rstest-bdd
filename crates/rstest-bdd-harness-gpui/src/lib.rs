//! GPUI harness adapter and attribute policy for `rstest-bdd`.
//!
//! This crate provides a GPUI-specific harness that wraps scenario execution
//! inside GPUI's test harness, and an attribute policy that emits
//! `#[gpui::test]` alongside `#[rstest::rstest]`.

mod gpui_harness;
mod policy;

pub use gpui_harness::GpuiHarness;
pub use policy::GpuiAttributePolicy;
