//! Compile-fail step definition with an invalid identifier.
use rstest_bdd_macros::given;

#[given("invalid identifier")]
fn 1invalid() {}

fn main() {}
