//! Unit tests for the `scenarios!` macro entry point.

#[cfg(unix)]
mod symlink_discovery {
    //! Unix symlink-discovery coverage.

    use std::{fs, os::unix::fs::symlink, path::Path};

    use tempfile::tempdir;

    use super::super::feature_discovery::collect_feature_files;

    #[test]
    fn collects_symlinked_feature_files_without_following_directory_loops() {
        let temp = tempdir().expect("test setup should succeed");
        let features_root = temp.path().join("features");
        fs::create_dir_all(features_root.join("nested")).expect("test setup should succeed");

        let feature_path = features_root.join("nested/example.feature");
        fs::write(&feature_path, "Feature: Example\n").expect("test setup should succeed");

        let symlink_path = features_root.join("symlink.feature");
        symlink(&feature_path, &symlink_path).expect("test setup should succeed");

        let relative_symlink_path = features_root.join("relative_link.feature");
        symlink(Path::new("nested/example.feature"), &relative_symlink_path)
            .expect("test setup should succeed");

        let loop_dir = features_root.join("loop");
        symlink(&features_root, &loop_dir).expect("test setup should succeed");

        let files =
            collect_feature_files(features_root.as_path()).expect("test setup should succeed");

        let mut expected = vec![feature_path, symlink_path, relative_symlink_path];
        expected.sort();
        assert_eq!(files, expected);
    }
}

#[cfg(not(unix))]
#[test]
fn collects_symlinked_feature_files_without_following_directory_loops() {
    assert!(cfg!(not(unix)));
}

mod empty_results {
    //! Coverage for the empty-expansion diagnostic.

    use proc_macro2::{Span, TokenStream as TokenStream2};
    use quote::quote;
    use rstest::{fixture, rstest};

    use super::super::{
        TagFilter,
        empty_results::{ExpansionOutput, check_empty_results},
    };
    use crate::parsing::tags::TagExpression;

    #[fixture]
    fn dir() -> syn::LitStr {
        let path = "tests/features/empty";
        syn::LitStr::new(path, Span::call_site())
    }

    fn fast_filter() -> TagFilter {
        TagFilter {
            expr: TagExpression::parse("@fast").expect("tag expression should parse"),
            span: Span::call_site(),
            raw: "@fast".to_owned(),
        }
    }

    fn render(errors: &[TokenStream2]) -> Vec<String> {
        errors.iter().map(ToString::to_string).collect()
    }

    #[rstest]
    #[case::no_filter(None, "no scenarios found in `tests/features/empty`")]
    #[case::unmatched_filter(Some(fast_filter()), "no scenarios matched tag expression `@fast`")]
    fn errors_when_nothing_is_emitted(
        dir: syn::LitStr,
        #[case] filter: Option<TagFilter>,
        #[case] expected: &str,
    ) {
        let mut errors = Vec::new();
        let output = ExpansionOutput {
            tests: &[],
            fallback_diagnostics: &TokenStream2::new(),
        };
        check_empty_results(output, &mut errors, filter.as_ref(), &dir);
        let rendered = render(&errors);
        let [message] = rendered.as_slice() else {
            panic!("expected one diagnostic, got {rendered:?}");
        };
        assert!(message.contains(expected), "{message}");
    }

    #[rstest]
    #[case::tests_generated(vec![quote! { fn generated() {} }], Vec::new(), TokenStream2::new())]
    #[case::earlier_error(Vec::new(), vec![quote! { compile_error!("earlier") }], TokenStream2::new())]
    #[case::adapter_diagnostic(
        Vec::new(),
        Vec::new(),
        quote! { compile_error!("invalid harness adapter") }
    )]
    fn stays_silent_when_something_was_emitted(
        dir: syn::LitStr,
        #[case] tests: Vec<TokenStream2>,
        #[case] errors: Vec<TokenStream2>,
        #[case] fallback_diagnostics: TokenStream2,
    ) {
        let before = render(&errors);
        let mut errors = errors;
        let output = ExpansionOutput {
            tests: &tests,
            fallback_diagnostics: &fallback_diagnostics,
        };
        check_empty_results(output, &mut errors, None, &dir);
        assert_eq!(render(&errors), before);
    }
}
