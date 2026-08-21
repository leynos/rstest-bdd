//! Compile-pass fixture proving the emitted rebuild-dependency tracking item
//! compiles in the staged trybuild crate root, and that several `#[scenario]`
//! bindings of the same feature file do not collide on the anonymous `const`
//! (one binding per file, Decision D0).
//!
//! The clippy-level denies are inert under `rustc` (the trybuild driver is not
//! the clippy driver), but they pin the intent that this fixture stay
//! lint-clean; the emitted tokens themselves are asserted lint-clean by the
//! token-shape tests in `rstest-bdd-macros`.

#![deny(warnings)]
#![deny(clippy::pedantic)]
#![deny(clippy::missing_docs_in_private_items)]

use rstest_bdd_macros::{given, scenario, scenarios, then, when};

// NOTE: these are plain `//` comments, not `///` docs — a doc comment
// immediately above a non-doc attribute is an `unused_doc_comment` warning
// under `#![deny(warnings)]`, which this fixture denies.

// Step shared by the bound features.
#[given("a precondition")]
fn precondition() {}

// Step shared by the bound features.
#[when("an action occurs")]
fn action() {}

// Step shared by the bound features.
#[then("a result is produced")]
fn result() {}

// First binding of `basic.feature`.
#[scenario(path = "basic.feature")]
fn first_binding() {}

// Second binding of the same file: the anonymous tracking `const` must not
// collide across scenarios sharing the file.
#[scenario(path = "basic.feature")]
fn second_binding() {}

// Directory autodiscovery through `scenarios!`: each discovered file emits
// its own tracking item alongside the generated module.
scenarios!("tests/features/auto");

fn main() {}