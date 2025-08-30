//! Compile-time tests for the procedural macros.

#[test]
fn step_macros_compile() {
    let t = trybuild::TestCases::new();
    t.pass("tests/fixtures/step_macros.rs");
    t.pass("tests/fixtures/step_macros_unicode.rs");
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
    t.compile_fail("tests/ui/implicit_fixture_missing.rs");
    t.compile_fail("tests/ui/placeholder_missing_param.rs");
    t.compile_fail("tests/fixtures/scenarios_missing_dir.rs");
    if cfg!(feature = "strict-compile-time-validation") {
        t.compile_fail("tests/fixtures/scenario_missing_step.rs");
        t.compile_fail("tests/fixtures/scenario_out_of_order.rs");
    } else {
        t.pass("tests/fixtures/scenario_missing_step.rs");
        t.pass("tests/fixtures/scenario_out_of_order.rs");
        t.compile_fail("tests/fixtures/scenario_missing_step_warning.rs");
    }
    if cfg!(feature = "compile-time-validation") {
        t.compile_fail("tests/fixtures/scenario_ambiguous_step.rs");
    }
}
