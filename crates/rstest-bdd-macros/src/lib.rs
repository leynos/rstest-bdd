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
mod scenario_state;
mod step_keyword;
mod utils;
mod validation;

pub(crate) use step_keyword::StepKeyword;

use proc_macro::TokenStream;
use proc_macro_error::proc_macro_error;

#[proc_macro_error]
#[proc_macro_attribute]
pub fn given(attr: TokenStream, item: TokenStream) -> TokenStream {
    macros::given(attr, item)
}

#[proc_macro_error]
#[proc_macro_attribute]
pub fn when(attr: TokenStream, item: TokenStream) -> TokenStream {
    macros::when(attr, item)
}

#[proc_macro_error]
#[proc_macro_attribute]
pub fn then(attr: TokenStream, item: TokenStream) -> TokenStream {
    macros::then(attr, item)
}

#[proc_macro_error]
#[proc_macro_attribute]
pub fn scenario(attr: TokenStream, item: TokenStream) -> TokenStream {
    macros::scenario(attr, item)
}

#[proc_macro_error]
#[proc_macro_derive(ScenarioState)]
pub fn derive_scenario_state(input: TokenStream) -> TokenStream {
    scenario_state::derive(input)
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
#[proc_macro_error]
#[proc_macro]
pub fn scenarios(input: TokenStream) -> TokenStream {
    macros::scenarios(input)
}

/// Derive `DataTableRow` for structs that should parse Gherkin rows.
///
/// The macro honours field-level overrides via `#[datatable(...)]` attributes
/// documented in the user guide.
#[proc_macro_error]
#[proc_macro_derive(DataTableRow, attributes(datatable))]
pub fn derive_data_table_row(input: TokenStream) -> TokenStream {
    datatable::derive_data_table_row(input)
}

/// Derive `DataTable` for tuple structs wrapping collections of rows.
///
/// The macro supports optional mapping hooks and row type inference as
/// described in the user guide.
#[proc_macro_error]
#[proc_macro_derive(DataTable, attributes(datatable))]
pub fn derive_data_table(input: TokenStream) -> TokenStream {
    datatable::derive_data_table(input)
}
