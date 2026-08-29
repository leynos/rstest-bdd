//! Compile-fail fixture for `Result` error types without `Display`.

use rstest_bdd_macros::when;

struct NotDisplay;

type Alias<T> = Result<T, NotDisplay>;

#[when("a direct error is not displayable")]
fn direct_error() -> Result<(), NotDisplay> { Err(NotDisplay) }

#[when("an alias error is not displayable")]
fn alias_error() -> Alias<()> { Err(NotDisplay) }

fn main() {}
