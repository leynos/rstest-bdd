//! Compile-fail fixture: `attributes` type in `scenarios!` does not implement
//! `AttributePolicy`.
use rstest_bdd_macros::scenarios;

struct NotAPolicy;

scenarios!(
    "tests/features/auto",
    attributes = NotAPolicy,
);

fn main() {}
