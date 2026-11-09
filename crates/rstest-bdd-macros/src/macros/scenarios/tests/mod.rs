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

    use super::super::{TagFilter, check_empty_results};
    use crate::parsing::tags::TagExpression;

    fn dir() -> syn::LitStr { syn::LitStr::new("tests/features/empty", Span::call_site()) }

    #[test]
    fn errors_naming_the_path_when_no_scenarios_exist() {
        let mut errors = Vec::new();
        check_empty_results(&[], &mut errors, None, &dir());
        let rendered: Vec<String> = errors.iter().map(ToString::to_string).collect();
        let [message] = rendered.as_slice() else {
            panic!("expected one diagnostic, got {rendered:?}");
        };
        assert!(
            message.contains("no scenarios found in `tests/features/empty`"),
            "{message}"
        );
    }

    #[test]
    fn errors_naming_the_expression_when_a_filter_matches_nothing() {
        let filter = TagFilter {
            expr: TagExpression::parse("@fast").expect("tag expression should parse"),
            span: Span::call_site(),
            raw: "@fast".to_owned(),
        };
        let mut errors = Vec::new();
        check_empty_results(&[], &mut errors, Some(&filter), &dir());
        let message = errors.first().map(ToString::to_string).unwrap_or_default();
        assert!(
            message.contains("no scenarios matched tag expression `@fast`"),
            "{message}"
        );
    }

    #[test]
    fn stays_silent_when_tests_or_errors_exist() {
        let generated: Vec<TokenStream2> = vec![quote! { fn generated() {} }];
        let mut errors = Vec::new();
        check_empty_results(&generated, &mut errors, None, &dir());
        assert!(errors.is_empty());

        let mut errors = vec![quote! { compile_error!("earlier") }];
        check_empty_results(&[], &mut errors, None, &dir());
        assert_eq!(errors.len(), 1);
    }
}
