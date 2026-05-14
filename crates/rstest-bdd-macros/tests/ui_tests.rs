//! Trybuild UI tests for generated step-registration code.

#[test]
fn ui() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/fixture_requirements_emitted.rs");
}
