//! Compile-pass fixture for every step-return dispatch classification.

#![deny(warnings)]

use rstest_bdd::StepResult;
use rstest_bdd_macros::when;

type Alias<T> = Result<T, &'static str>;

#[when("a unit value is returned")]
fn unit() {}

#[when("a plain value is returned")]
fn value() -> u8 { 1 }

#[when("a result is returned")]
fn result() -> Result<(), &'static str> { Ok(()) }

#[when("an alias is returned")]
fn alias() -> Alias<()> { Ok(()) }

#[when("a StepResult is returned")]
fn step_result() -> StepResult<(), &'static str> { Ok(()) }

#[when("an anyhow result is returned")]
fn anyhow_result() -> anyhow::Result<()> { Ok(()) }

#[when("an io result is returned")]
fn io_result() -> std::io::Result<()> { Ok(()) }

fn main() {}
