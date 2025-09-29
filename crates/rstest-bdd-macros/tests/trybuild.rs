//! Compile-time tests for the procedural macros.

use rstest::rstest;

#[path = "trybuild_support.rs"]
mod support;

#[cfg(test)]
#[path = "support/trybuild_helpers.rs"]
mod helper_tests;

use support::FixturePath;

#[rstest]
#[case::step_macros(FixturePath::new("tests/fixtures/step_macros.rs"))]
#[case::step_macros_unicode(FixturePath::new("tests/fixtures/step_macros_unicode.rs"))]
#[case::scenario_single_match(FixturePath::new("tests/fixtures/scenario_single_match.rs"))]
#[cfg_attr(
    not(feature = "strict-compile-time-validation"),
    case::scenario_missing_step(FixturePath::new("tests/fixtures/scenario_missing_step.rs"))
)]
#[cfg_attr(
    not(feature = "strict-compile-time-validation"),
    case::scenario_out_of_order(FixturePath::new("tests/fixtures/scenario_out_of_order.rs"))
)]
fn trybuild_fixtures_pass(#[case] fixture: FixturePath<'static>) {
    let t = trybuild::TestCases::new();
    t.pass(fixture.as_str());
}

#[rstest]
#[case::scenario_missing_file(FixturePath::new("tests/fixtures/scenario_missing_file.rs"))]
#[case::step_macros_invalid_identifier(FixturePath::new(
    "tests/fixtures/step_macros_invalid_identifier.rs"
))]
#[case::step_tuple_pattern(FixturePath::new("tests/fixtures/step_tuple_pattern.rs"))]
#[case::step_struct_pattern(FixturePath::new("tests/fixtures/step_struct_pattern.rs"))]
#[case::step_nested_pattern(FixturePath::new("tests/fixtures/step_nested_pattern.rs"))]
#[case::datatable_wrong_type(FixturePath::new("tests/ui/datatable_wrong_type.rs"))]
#[case::datatable_duplicate(FixturePath::new("tests/ui/datatable_duplicate.rs"))]
#[case::datatable_duplicate_attr(FixturePath::new("tests/ui/datatable_duplicate_attr.rs"))]
#[case::datatable_after_docstring(FixturePath::new("tests/ui/datatable_after_docstring.rs"))]
#[case::placeholder_missing_param(FixturePath::new("tests/ui/placeholder_missing_param.rs"))]
#[case::implicit_fixture_missing(FixturePath::new("tests/ui/implicit_fixture_missing.rs"))]
#[case::placeholder_missing_params(FixturePath::new("tests/ui/placeholder_missing_params.rs"))]
#[case::scenarios_missing_dir(FixturePath::new("tests/fixtures/scenarios_missing_dir.rs"))]
#[cfg_attr(
    feature = "strict-compile-time-validation",
    case::scenario_missing_step(FixturePath::new("tests/fixtures/scenario_missing_step.rs"))
)]
#[cfg_attr(
    feature = "strict-compile-time-validation",
    case::scenario_out_of_order(FixturePath::new("tests/fixtures/scenario_out_of_order.rs"))
)]
#[cfg_attr(
    feature = "compile-time-validation",
    case::scenario_ambiguous_step(FixturePath::new("tests/fixtures/scenario_ambiguous_step.rs"))
)]
fn trybuild_fixtures_compile_fail(#[case] fixture: FixturePath<'static>) {
    let t = trybuild::TestCases::new();
    t.compile_fail(fixture.as_str());
}

#[cfg(not(feature = "strict-compile-time-validation"))]
#[test]
fn scenario_missing_step_emits_warning() {
    let t = trybuild::TestCases::new();
    compile_fail_missing_step_warning(&t);
}

#[cfg(not(feature = "strict-compile-time-validation"))]
fn compile_fail_missing_step_warning(t: &trybuild::TestCases) {
    support::compile_fail_with_normalised_output(
        t,
        FixturePath::new("tests/fixtures/scenario_missing_step_warning.rs"),
        &[
            support::strip_nightly_macro_backtrace_hint,
            support::normalise_fixture_paths,
        ],
    );
}
