//! Tests for resolving selected step-library marker paths.

use syn::parse_quote;

#[test]
fn global_marker_special_case_is_limited_to_the_runtime_crate() {
    let builtin: syn::Path = parse_quote!(rstest_bdd::global);
    let user_library: syn::Path = parse_quote!(steps::global);

    let builtin_tokens = super::super::library_marker_path(&builtin).to_string();
    let user_tokens = super::super::library_marker_path(&user_library).to_string();

    assert!(
        builtin_tokens.contains("global :: STEP_LIBRARY"),
        "{builtin_tokens}"
    );
    assert!(
        !builtin_tokens.contains("__RSTEST_BDD_STEP_LIBRARY_global"),
        "{builtin_tokens}"
    );
    assert!(
        user_tokens.contains("__RSTEST_BDD_STEP_LIBRARY_global"),
        "{user_tokens}"
    );
    assert!(
        !user_tokens.contains("global :: STEP_LIBRARY"),
        "{user_tokens}"
    );
}
