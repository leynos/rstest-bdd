//! Compile-fail fixture pinning W002 and the GPUI attribute consequence.
//!
//! The first scenario supplies a non-canonical GPUI harness and no attribute
//! policy: the canonical spelling would apply `#[gpui::test]`, so the missing
//! policy is reported as W002. The second supplies a non-canonical GPUI policy
//! in the `attributes` slot, which is W001 and does state the lost attribute.
#![deny(deprecated)]

use rstest_bdd_macros::{given, scenario, then, when};

mod nested {
    //! A two-level re-export of the GPUI harness crate.

    pub mod deep {
        pub use rstest_bdd_harness_gpui;
    }
}

#[given("a precondition")]
fn precondition() {}

#[when("an action occurs")]
fn action() {}

#[then("a result is produced")]
fn result() {}

#[scenario(
    path = "basic.feature",
    harness = nested::deep::rstest_bdd_harness_gpui::GpuiHarness,
)]
fn with_nested_gpui_harness() {}

#[scenario(
    path = "basic.feature",
    attributes = nested::deep::rstest_bdd_harness_gpui::GpuiAttributePolicy,
)]
fn with_nested_gpui_policy() {}

/// The user already wrote `#[gpui::test]`, so the attribute is present and
/// the missing-policy claim is false: this must render W001 without it.
#[scenario(
    path = "basic.feature",
    harness = nested::deep::rstest_bdd_harness_gpui::GpuiHarness,
)]
#[gpui::test]
fn with_nested_gpui_harness_and_user_attribute() {}

const _: &str = include_str!("basic.feature");

fn main() {}