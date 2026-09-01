//! Unit tests for the `#[harness_context]` classifier.
//!
//! Every case drives the full `extract_args` pipeline, not the classifier in
//! isolation, because the contract under test is the observable classification
//! of a step signature: three request spellings must converge on one fixture
//! key, and every misuse must produce a targeted diagnostic.

use std::collections::HashSet;

use googletest::prelude::*;
use rstest::rstest;

use super::super::super::{Arg, ExtractedArgs, extract_args};

/// Parse a step function source string and classify its arguments.
///
/// `placeholders` names the step-pattern placeholders the signature should be
/// matched against, mirroring what the wrapper codegen passes into
/// `extract_args`.
fn classify_step(src: &str, placeholders: &[&str]) -> syn::Result<ExtractedArgs> {
    let mut func: syn::ItemFn = syn::parse_str(src)?;
    let mut set: HashSet<String> = placeholders.iter().map(|p| (*p).to_string()).collect();
    extract_args(&mut func, &mut set)
}

#[rstest]
#[case::shared("&TestCtx")]
#[case::mutable("&mut TestCtx")]
#[googletest::test]
fn marker_binds_the_reserved_fixture_key_and_preserves_context_type(
    #[case] context_ty: &str,
) -> googletest::Result<()> {
    let extracted = classify_step(
        &format!("fn s(#[harness_context] ctx: {context_ty}) {{}}"),
        &[],
    )?;
    let expected_ty: syn::Type = syn::parse_str(context_ty)?;

    verify_that!(
        extracted.args,
        elements_are![matches_pattern!(Arg::Fixture {
            name: displays_as(eq(rstest_bdd_policy::HARNESS_CONTEXT_FIXTURE)),
            pat: displays_as(eq("ctx")),
            ty: eq(&expected_ty),
            ..
        })]
    )
}

#[test]
fn marker_and_from_spelling_produce_equal_arguments() {
    use pretty_assertions::assert_eq;

    let via_marker =
        classify_step("fn s(#[harness_context] ctx: &TestCtx) {}", &[]).expect("marker form");
    let via_from = classify_step(
        "fn s(#[from(rstest_bdd_harness_context)] ctx: &TestCtx) {}",
        &[],
    )
    .expect("from form");

    assert_eq!(via_marker.args, via_from.args);
}

#[test]
fn parameter_named_after_the_reserved_key_produces_the_same_fixture() {
    let via_marker =
        classify_step("fn s(#[harness_context] ctx: &TestCtx) {}", &[]).expect("marker form");
    let by_name = classify_step("fn s(rstest_bdd_harness_context: &TestCtx) {}", &[])
        .expect("parameter-named form");

    // The parameter-named spelling binds the same fixture key; the parameter
    // identifier itself differs (`rstest_bdd_harness_context` vs `ctx`), so
    // assert on the resolved fixture name rather than full equality.
    assert_eq!(
        via_marker
            .args
            .iter()
            .map(|arg| match arg {
                Arg::Fixture { name, .. } => name.to_string(),
                _ => panic!("expected a fixture argument"),
            })
            .collect::<Vec<_>>(),
        by_name
            .args
            .iter()
            .map(|arg| match arg {
                Arg::Fixture { name, .. } => name.to_string(),
                _ => panic!("expected a fixture argument"),
            })
            .collect::<Vec<_>>(),
    );
}

#[gtest]
fn marker_coexists_with_placeholders_and_fixtures() -> googletest::Result<()> {
    let extracted = classify_step(
        "fn s(count: u32, #[harness_context] ctx: &TestCtx, pool: DbPool) {}",
        &["count"],
    )?;

    expect_that!(extracted.args, len(eq(3)));
    expect_that!(
        extracted.args.as_slice(),
        elements_are![
            matches_pattern!(Arg::Step { .. }),
            matches_pattern!(Arg::Fixture {
                name: displays_as(eq("rstest_bdd_harness_context")),
                ..
            }),
            matches_pattern!(Arg::Fixture {
                name: displays_as(eq("pool")),
                ..
            }),
        ]
    );
    Ok(())
}

#[rstest]
#[googletest::test]
fn reserved_fixture_key_parses_as_an_identifier() -> googletest::Result<()> {
    verify_that!(
        syn::parse_str::<syn::Ident>(rstest_bdd_policy::HARNESS_CONTEXT_FIXTURE),
        ok(anything())
    )
}

#[rstest]
#[case::with_from(
    "#[harness_context] #[from(x)] c: &C",
    "cannot be combined with `#[from]`"
)]
#[case::with_datatable(
    "#[harness_context] #[datatable] c: &C",
    "cannot be combined with `#[datatable]`"
)]
#[case::with_step_args(
    "#[harness_context] #[step_args] c: &C",
    "cannot be combined with `#[step_args]`"
)]
#[case::argued("#[harness_context(gpui)] c: &C", "does not take arguments")]
#[case::duplicated(
    "#[harness_context] #[harness_context] c: &C",
    "duplicate `#[harness_context]`"
)]
#[googletest::test]
fn rejects_conflicting_annotations(
    #[case] param: &str,
    #[case] expected: &str,
) -> googletest::Result<()> {
    let result = classify_step(&format!("fn s({param}) {{}}"), &[]);

    verify_that!(result, err(displays_as(contains_substring(expected))))
}

#[rstest]
#[case("count")]
#[case("_count")]
fn rejects_the_marker_on_a_placeholder_bound_parameter(#[case] parameter: &str) {
    let result = classify_step(
        &format!("fn s(#[harness_context] {parameter}: &Ctx) {{}}"),
        &["count"],
    );

    let Err(err) = result else {
        panic!("a placeholder-bound marker parameter must be rejected");
    };
    let msg = err.to_string();
    assert!(
        msg.contains("placeholder") && msg.contains("count"),
        "expected a diagnostic naming the placeholder, got: {msg}"
    );
}
