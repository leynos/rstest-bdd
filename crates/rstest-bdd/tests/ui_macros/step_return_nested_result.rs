//! Compile-fail fixture for unhinted nested `Result` step returns.

use rstest_bdd_macros::when;

#[when("a nested result is returned")]
fn nested_result() -> Result<Result<(), &'static str>, &'static str> { Ok(Ok(())) }

fn main() {}
