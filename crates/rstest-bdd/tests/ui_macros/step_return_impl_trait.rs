//! Compile-fail fixture for unhinted opaque step returns.

use rstest_bdd_macros::when;

#[when("an opaque result is returned")]
fn opaque_result() -> impl std::fmt::Debug { Ok::<(), &'static str>(()) }

fn main() {}
