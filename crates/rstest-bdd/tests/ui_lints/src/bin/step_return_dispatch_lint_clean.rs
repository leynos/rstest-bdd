//! Clippy UI fixture for type-directed step return normalization.

#![deny(warnings)]

use rstest_bdd::StepResult;
use rstest_bdd_macros::when;

type Alias<T> = Result<T, &'static str>;

#[when("a lint-clean unit step runs")]
fn unit() {}

#[when("a lint-clean value step runs")]
fn value() -> u8 { 1 }

#[when("a lint-clean Result step runs")]
fn result() -> Result<(), &'static str> { Ok(()) }

#[when("a lint-clean alias step runs")]
fn alias() -> Alias<()> { Ok(()) }

#[when("a lint-clean StepResult step runs")]
fn step_result() -> StepResult<(), &'static str> { Ok(()) }

#[when("a lint-clean anyhow step runs")]
fn anyhow_result() -> anyhow::Result<()> { Ok(()) }

#[when("a lint-clean IO step runs")]
fn io_result() -> std::io::Result<()> { Ok(()) }

fn main() {}
