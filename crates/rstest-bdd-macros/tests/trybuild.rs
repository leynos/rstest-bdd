//! Compile-time tests for the procedural macros.

#[test]
fn step_macros_compile() {
    let t = trybuild::TestCases::new();
    t.pass("tests/fixtures/step_macros.rs");
    // `scenarios!` should succeed when the directory exists.
    // t.pass("tests/fixtures/scenarios_autodiscovery.rs");
    t.compile_fail("tests/fixtures/scenario_missing_file.rs");
    t.compile_fail("tests/fixtures/step_tuple_pattern.rs");
    t.compile_fail("tests/fixtures/step_struct_pattern.rs");
    t.compile_fail("tests/fixtures/step_nested_pattern.rs");
    t.compile_fail("tests/ui/outline_missing_examples.rs");
    t.compile_fail("tests/ui/outline_empty_examples.rs");
    t.compile_fail("tests/ui/outline_missing_column.rs");
    t.compile_fail("tests/ui/outline_duplicate_headers.rs");
    t.compile_fail("tests/fixtures/scenarios_missing_dir.rs");
}
