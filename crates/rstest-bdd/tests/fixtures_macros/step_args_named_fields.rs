//! Compile-pass fixture for named and normalized `StepArgs` fields.

use rstest_bdd_macros::{StepArgs, when};

#[derive(StepArgs)]
struct Transfer {
    #[step_args(placeholder = "sender")]
    from: String,
    #[step_args(trim, parse_with = parse_amount)]
    amount: u64,
    recipient: String,
}

fn parse_amount(value: &str) -> Result<u64, std::num::ParseIntError> { value.parse() }

#[when("{sender} transfers {amount} to {recipient}")]
fn transfer(#[step_args] _transfer: Transfer) {}

fn main() {}
