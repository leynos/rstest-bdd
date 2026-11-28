//! The facts the `#[scenario]` boundary hands to policy diagnostics.
//!
//! Gathering them here keeps `try_scenario` from growing and keeps the
//! `rstest` rule in one place.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;

use super::args::ScenarioArgs;

/// Build the diagnostics this `#[scenario]` expansion owns.
///
/// The macro cannot evaluate trait methods, so the function's signature and
/// attributes are all the evidence the decision has. An error replaces the
/// generated test, because `#[scenario]` always generates exactly one.
///
/// Warnings are returned, not emitted: the caller emits them only after the
/// boundary's last fallible step succeeds, so a rejection cannot be
/// accompanied by a warning on either channel.
pub(super) fn build_scenario_diagnostics(
    args: &ScenarioArgs,
    item_fn: &syn::ItemFn,
) -> std::result::Result<(crate::codegen::SharedPolicyDiagnostics, TokenStream2), TokenStream> {
    let facts = crate::codegen::ScenarioFacts {
        is_async: item_fn.sig.asyncness.is_some(),
        has_executor_attr: has_executor_attr(&item_fn.attrs),
        has_gpui_test_attr: has_gpui_test_attr(&item_fn.attrs),
    };
    // This rejection predates the diagnostics and is an error, so it must be
    // decided before either channel speaks (decision D9).
    if facts.is_async && args.harness.is_some() {
        return Err(crate::codegen::scenario::reject_async_harness_error().into());
    }
    let resolutions =
        crate::codegen::SharedPolicyDiagnostics::for_scenario(crate::codegen::ScenarioRequest {
            harness: args.harness.as_ref(),
            attributes: args.attributes.as_ref(),
            facts,
        });
    if let Some(errors) = resolutions.errors() {
        return Err(errors.into());
    }
    let warnings = resolutions.warning_tokens();
    Ok((resolutions, warnings))
}

/// Whether a user-written attribute satisfies `rstest`'s asynchronous-test
/// rule.
///
/// `rstest` accepts an `async` test when it carries an explicit
/// `#[test_attr(...)]` or any attribute whose final path segment is `test`
/// (`rstest_macros` 0.26.1: `extract_test_attr` and `first_valid_test_attrs`).
/// Mirroring that rule exactly is what stops a diagnostic from rejecting a
/// program `rstest` accepts.
///
/// # Examples
///
/// ```rust,ignore
/// let attrs: Vec<syn::Attribute> = vec![syn::parse_quote!(#[tokio::test])];
/// assert!(has_executor_attr(&attrs));
/// ```
pub(super) fn has_executor_attr(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.path().is_ident("test_attr")
            || attr
                .path()
                .segments
                .last()
                .is_some_and(|segment| segment.ident == "test")
    })
}

/// Whether the user already wrote `#[gpui::test]`.
///
/// The generator deduplicates against this attribute and no other, so it is
/// the only user attribute that can make an "was not applied" claim false.
///
/// # Examples
///
/// ```rust,ignore
/// let attrs: Vec<syn::Attribute> = vec![syn::parse_quote!(#[gpui::test])];
/// assert!(has_gpui_test_attr(&attrs));
/// ```
pub(super) fn has_gpui_test_attr(attrs: &[syn::Attribute]) -> bool {
    // Reuse the generator's predicate, so the deduplication this suppresses a
    // claim for is by construction the deduplication that will happen.
    attrs
        .iter()
        .any(crate::codegen::scenario::test_attrs::is_gpui_test_attr)
}

#[cfg(test)]
mod tests {
    //! Table tests for the `rstest` executor-attribute rule.

    use rstest::rstest;

    use super::{has_executor_attr, has_gpui_test_attr};

    fn attrs(attribute: &str) -> Vec<syn::Attribute> {
        let source = format!("{attribute} fn example() {{}}");
        match syn::parse_str::<syn::ItemFn>(&source) {
            Ok(item) => item.attrs,
            Err(error) => panic!("{source} should parse: {error}"),
        }
    }

    #[rstest]
    #[case("#[gpui::test]", true)]
    #[case("#[gpui::test(flavor = \"current_thread\")]", true)]
    #[case("#[test]", true)]
    #[case("#[tokio::test]", true)]
    #[case("#[gpui::test]", true)]
    #[case("#[tokio::test(flavor = \"current_thread\")]", true)]
    #[case("#[test_attr(tokio::test)]", true)]
    #[case("#[test_case(1)]", false)]
    #[case("#[should_panic]", false)]
    #[case("#[serial]", false)]
    fn recognizes_an_executor_attribute(#[case] attribute: &str, #[case] expected: bool) {
        assert_eq!(
            has_executor_attr(&attrs(attribute)),
            expected,
            "{attribute} must be classified as an executor attribute: {expected}"
        );
    }

    #[rstest]
    #[case("#[gpui::test]", true)]
    #[case("#[gpui::test(flavor = \"current_thread\")]", true)]
    #[case("#[test]", false)]
    #[case("#[tokio::test]", false)]
    #[case("#[should_panic]", false)]
    fn recognizes_a_user_gpui_test(#[case] attribute: &str, #[case] expected: bool) {
        assert_eq!(
            has_gpui_test_attr(&attrs(attribute)),
            expected,
            "{attribute} GPUI-attribute classification must be {expected}"
        );
    }

    #[test]
    fn an_attribute_free_function_has_no_executor() {
        assert!(
            !has_executor_attr(&[]),
            "no attributes means no executor attribute"
        );
    }
}
