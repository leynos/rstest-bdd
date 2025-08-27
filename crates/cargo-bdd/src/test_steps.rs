use rstest_bdd_macros::{given, then, when};

#[given("I have cukes")]
fn have_cukes() {}

#[when("I eat them")]
fn eat_them() {}

#[then("I should be satisfied")]
fn satisfied() {}

#[given("unused step")]
fn unused() {}

#[given("duplicate step")]
fn dup_one() {}

#[given("duplicate step")]
fn dup_two() {}
