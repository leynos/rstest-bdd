//! Compile-time fixture verifying scenario returns must be unit results.
use rstest_bdd_macros::scenario;

#[scenario(path = "basic.feature")]
fn scenario_result_payload() -> Result<u8, &'static str> {
    Ok(1)
}

const _: &str = include_str!("basic.feature");

fn main() {}
