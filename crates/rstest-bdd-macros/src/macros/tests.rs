//! Unit tests for step-attribute diagnostic help text.

use super::signature_error_help;
use crate::{StepKeyword, codegen::wrapper::args::classify::DUPLICATE_DATATABLE_ERROR};

#[test]
fn duplicate_datatable_help_names_the_remedy() {
    // Pins the `DUPLICATE_DATATABLE_ERROR` -> help-text mapping directly,
    // independent of the trybuild fixture (which only runs under `cargo test`,
    // not `make test`/nextest).
    let help = signature_error_help(DUPLICATE_DATATABLE_ERROR, StepKeyword::Given);
    assert_eq!(help, "Remove one of the DataTable parameters.");
}
