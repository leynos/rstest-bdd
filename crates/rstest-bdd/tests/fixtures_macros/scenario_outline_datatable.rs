//! Compile-pass fixture: a Scenario Outline whose Background and step carry
//! data tables compiles on stable Rust. Every Examples row's steps live in a
//! `const`, so the generated table literals must avoid range indexing.
use rstest_bdd_macros::{given, scenario, when};

#[given("a background table:")]
fn background_table(datatable: Vec<Vec<String>>) { let _ = datatable; }

#[when("an outline table:")]
fn outline_table(datatable: Vec<Vec<String>>) { let _ = datatable; }

#[scenario(path = "outline_datatable.feature")]
fn outline_datatable(item: String) { let _ = item; }

// Compile-time guard: fail fast if the feature path changes.
const _: &str = include_str!("outline_datatable.feature");

fn main() {}
