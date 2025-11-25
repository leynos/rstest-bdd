//! Compile-fail fixture asserting ambiguous step detection for duplicate `given` steps.
// compile-flags: --cfg feature="compile-time-validation"
use rstest_bdd_macros::{given, scenario};

#[given("a step")]
fn first() {}

#[given("a step")]
fn second() {}

#[scenario(path = "../features/macros/ambiguous.feature")]
fn ambiguous() {}

fn main() {}
