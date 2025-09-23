//! Compile-time tests for the procedural macros.

use std::fs;
use std::io;
use std::panic::{self, AssertUnwindSafe};

#[test]
fn step_macros_compile() {
    let t = trybuild::TestCases::new();
    t.pass("tests/fixtures/step_macros.rs");
    t.pass("tests/fixtures/step_macros_unicode.rs");
    t.pass("tests/fixtures/scenario_single_match.rs");
    // `scenarios!` should succeed when the directory exists.
    // t.pass("tests/fixtures/scenarios_autodiscovery.rs");
    t.compile_fail("tests/fixtures/scenario_missing_file.rs");
    t.compile_fail("tests/fixtures/step_macros_invalid_identifier.rs");
    t.compile_fail("tests/fixtures/step_tuple_pattern.rs");
    t.compile_fail("tests/fixtures/step_struct_pattern.rs");
    t.compile_fail("tests/fixtures/step_nested_pattern.rs");
    t.compile_fail("tests/ui/datatable_wrong_type.rs");
    t.compile_fail("tests/ui/datatable_duplicate.rs");
    t.compile_fail("tests/ui/datatable_duplicate_attr.rs");
    t.compile_fail("tests/ui/datatable_after_docstring.rs");
    t.compile_fail("tests/ui/placeholder_missing_param.rs");
    t.compile_fail("tests/ui/implicit_fixture_missing.rs");
    t.compile_fail("tests/ui/placeholder_missing_params.rs");
    t.compile_fail("tests/fixtures/scenarios_missing_dir.rs");
    if cfg!(feature = "strict-compile-time-validation") {
        t.compile_fail("tests/fixtures/scenario_missing_step.rs");
        t.compile_fail("tests/fixtures/scenario_out_of_order.rs");
    } else {
        t.pass("tests/fixtures/scenario_missing_step.rs");
        t.pass("tests/fixtures/scenario_out_of_order.rs");
        compile_fail_missing_step_warning(&t);
    }
    if cfg!(feature = "compile-time-validation") {
        t.compile_fail("tests/fixtures/scenario_ambiguous_step.rs");
    }
}

fn compile_fail_missing_step_warning(t: &trybuild::TestCases) {
    const TEST_PATH: &str = "tests/fixtures/scenario_missing_step_warning.rs";

    match panic::catch_unwind(AssertUnwindSafe(|| t.compile_fail(TEST_PATH))) {
        Ok(()) => (),
        Err(panic) => {
            if normalise_missing_step_warning_output().unwrap_or(false) {
                return;
            }

            panic::resume_unwind(panic);
        }
    }
}

fn normalise_missing_step_warning_output() -> io::Result<bool> {
    const WIP_PATH: &str = "target/tests/wip/scenario_missing_step_warning.stderr";
    const EXPECTED_PATH: &str = "tests/fixtures/scenario_missing_step_warning.stderr";

    let actual = fs::read_to_string(WIP_PATH)?;
    let expected = fs::read_to_string(EXPECTED_PATH)?;

    if normalise_nightly_parenthetical(&actual) == normalise_nightly_parenthetical(&expected) {
        let _ = fs::remove_file(WIP_PATH);
        return Ok(true);
    }

    Ok(false)
}

fn normalise_nightly_parenthetical(text: &str) -> String {
    text.replace(
        " (in Nightly builds, run with -Z macro-backtrace for more info)",
        "",
    )
}
