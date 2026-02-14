//! Compile-fail fixture: `harness` type in `scenarios!` does not implement
//! `HarnessAdapter`.
use rstest_bdd_macros::scenarios;

struct NotAHarness;

scenarios!(
    "tests/features/auto",
    harness = NotAHarness,
);

fn main() {}
