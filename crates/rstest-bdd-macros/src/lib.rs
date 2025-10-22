#![cfg_attr(docsrs, feature(doc_cfg))]
//! Attribute macros enabling Behaviour-Driven testing with `rstest`.
//!
//! # Feature flags
//! - `compile-time-validation`: registers steps at compile time and attaches
//!   spans for diagnostics.
//! - `strict-compile-time-validation`: escalates missing or ambiguous steps to
//!   compile errors; implies `compile-time-validation`.
//!
//! Both features are disabled by default.

mod codegen;
mod datatable;
mod macros;
mod parsing;
mod pattern;
mod step_keyword;
mod utils;
mod validation;

pub(crate) use step_keyword::StepKeyword;

use proc_macro::TokenStream;
use proc_macro_error::entry_point;
use std::panic::AssertUnwindSafe;

// Keep `proc_macro_error::abort!` diagnostics wired up without relying on the
// attribute helper, which drops doc comments after expansion on some
// toolchains.
fn run_with_macro_errors<F>(f: F) -> TokenStream
where
    F: FnOnce() -> TokenStream,
{
    entry_point(AssertUnwindSafe(f), false)
}

/// Attribute macro registering a step definition for the `Given` keyword.
///
/// # Examples
/// ```ignore
/// use rstest_bdd_macros::given;
///
/// #[given("a configured database")]
/// fn a_configured_database() {}
/// ```
#[proc_macro_attribute]
pub fn given(attr: TokenStream, item: TokenStream) -> TokenStream {
    run_with_macro_errors(move || macros::given(attr, item))
}

/// Attribute macro registering a step definition for the `When` keyword.
///
/// # Examples
/// ```ignore
/// use rstest_bdd_macros::when;
///
/// #[when("the user logs in")]
/// fn the_user_logs_in() {}
/// ```
#[proc_macro_attribute]
pub fn when(attr: TokenStream, item: TokenStream) -> TokenStream {
    run_with_macro_errors(move || macros::when(attr, item))
}

/// Attribute macro registering a step definition for the `Then` keyword.
///
/// # Examples
/// ```ignore
/// use rstest_bdd_macros::then;
///
/// #[then("a success message is shown")]
/// fn a_success_message_is_shown() {}
/// ```
#[proc_macro_attribute]
pub fn then(attr: TokenStream, item: TokenStream) -> TokenStream {
    run_with_macro_errors(move || macros::then(attr, item))
}

/// Attribute macro binding a test function to a single Gherkin scenario.
///
/// Selector semantics:
/// - Supply either `index = N` (zero-based) or `name = "Scenario title"` to
///   disambiguate when the feature defines multiple scenarios.
/// - When omitted, the macro targets the first scenario in the feature file.
///
/// Tag filtering:
/// - Provide `tags = "expr"` to keep only scenarios whose tag sets satisfy the
///   expression before applying selectors.
/// - Expressions accept case-sensitive tag names combined with `not`, `and`,
///   and `or`, following the precedence `not` > `and` > `or`. Parentheses may
///   be used to override the default binding.
///
/// Example:
/// ```ignore
/// use rstest_bdd_macros::scenario;
///
/// #[scenario(
///     "tests/features/filtering.feature",
///     tags = "@fast and not (@wip or @flaky)"
/// )]
/// fn fast_stable_cases() {}
/// ```
#[proc_macro_attribute]
pub fn scenario(attr: TokenStream, item: TokenStream) -> TokenStream {
    run_with_macro_errors(move || macros::scenario(attr, item))
}

/// Discover all `.feature` files under the given directory and generate one
/// test per Gherkin `Scenario`.
///
/// Path semantics:
/// - The `dir` argument must be a string literal.
/// - It is resolved relative to `CARGO_MANIFEST_DIR` at macro-expansion time.
///
/// Expansion:
/// - Emits a module named after `dir` (sanitized) containing one test function
///   per discovered scenario.
/// - Each generated test executes the matched steps via the registered
///   `#[given]`, `#[when]`, and `#[then]` functions.
///
/// Example:
/// ```ignore
/// use rstest_bdd_macros::{given, when, then, scenarios};
///
/// # #[given("a precondition")] fn precondition() {}
/// # #[when("an action occurs")] fn action() {}
/// # #[then("events are recorded")] fn events_recorded() {}
/// scenarios!("tests/features/auto");
/// ```
///
/// Errors:
/// - Emits a compile error if the directory does not exist, contains no
///   `.feature` files, or if parsing fails.
#[proc_macro]
pub fn scenarios(input: TokenStream) -> TokenStream {
    run_with_macro_errors(move || macros::scenarios(input))
}

/// Derive `DataTableRow` for structs that should parse Gherkin rows.
///
/// The macro honours field-level overrides via `#[datatable(...)]` attributes
/// documented in the user guide.
#[proc_macro_derive(DataTableRow, attributes(datatable))]
pub fn derive_data_table_row(input: TokenStream) -> TokenStream {
    run_with_macro_errors(move || datatable::derive_data_table_row(input))
}

/// Derive `DataTable` for tuple structs wrapping collections of rows.
///
/// The macro supports optional mapping hooks and row type inference as
/// described in the user guide.
#[proc_macro_derive(DataTable, attributes(datatable))]
pub fn derive_data_table(input: TokenStream) -> TokenStream {
    run_with_macro_errors(move || datatable::derive_data_table(input))
}
