//! UI compile-fail fixture for missing multiple placeholder parameters.
use rstest_bdd_macros::given;

#[given("numbers {a} then {b} and finally {c}")]
fn missing_multi() {}

#[given("value {v:u32}")]
fn missing_single() {}

#[given("sum {x} and {y}")]
fn missing_two_of_three(_z: u32) {}

fn main() {}
