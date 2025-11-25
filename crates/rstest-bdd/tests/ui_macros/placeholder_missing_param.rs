//! UI compile-fail fixture for missing placeholder parameter in step function.
use rstest_bdd_macros::given;

#[given("a number {count:u32}")]
fn step() {}

fn main() {}
