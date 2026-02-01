//! Compile-time fixture verifying scenario StepResult payloads must be unit.

use rstest_bdd_macros::scenario;

#[scenario(path = "basic.feature")]
fn scenario_step_result_payload() -> rstest_bdd::StepResult<u32, &'static str> {
    Ok(1)
}

const _: &str = include_str!("basic.feature");

fn main() {}
