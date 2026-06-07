//! Trybuild UI tests for generated step-registration code.

#[test]
fn ui() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/fixture_requirements_emitted.rs");
    t.compile_fail("tests/ui/mixed_mutability_fixtures.rs");
    t.compile_fail("tests/ui/two_mutable_fixtures.rs");
}
