//! Tokio harness adapter and attribute policy for `rstest-bdd`.
//!
//! This crate provides a Tokio-specific harness that wraps scenario execution
//! inside a current-thread Tokio runtime, and an attribute policy that emits
//! `#[tokio::test(flavor = "current_thread")]` alongside `#[rstest::rstest]`.

mod policy;
mod tokio_harness;

pub use policy::TokioAttributePolicy;
pub use tokio_harness::TokioHarness;
