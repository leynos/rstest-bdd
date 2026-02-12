//! Compile-pass fixture validating that `scenarios!` accepts `harness` and
//! `attributes` parameters with valid types.
use rstest_bdd_macros::scenarios;

scenarios!(
    "tests/features/auto",
    harness = rstest_bdd_harness::StdHarness,
    attributes = rstest_bdd_harness::DefaultAttributePolicy,
);

fn main() {}
