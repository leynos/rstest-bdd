//! The facts the `#[scenario]` boundary hands to policy diagnostics.
//!
//! Gathering them here keeps `try_scenario` from growing and keeps the
//! `rstest` rule in one place.

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

#[cfg(test)]
mod tests {
    //! Table tests for the `rstest` executor-attribute rule.

    use rstest::rstest;

    use super::has_executor_attr;

    fn attrs(attribute: &str) -> Vec<syn::Attribute> {
        let source = format!("{attribute} fn example() {{}}");
        match syn::parse_str::<syn::ItemFn>(&source) {
            Ok(item) => item.attrs,
            Err(error) => panic!("{source} should parse: {error}"),
        }
    }

    #[rstest]
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

    #[test]
    fn an_attribute_free_function_has_no_executor() {
        assert!(
            !has_executor_attr(&[]),
            "no attributes means no executor attribute"
        );
    }
}
