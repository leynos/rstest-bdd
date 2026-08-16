//! Token-shape tests for the Cargo rebuild-dependency tracking binding
//! (`ExecPlan` milestone 1d).
//!
//! These tests pin the *exact* emitted tokens for the tracking item, so a
//! future change that keeps a substring-like resemblance but breaks the
//! contract — for example an absolute path smuggled into the literal, or a
//! wrong relative offset — fails loudly. Substring presence is not enough:
//! `include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/", "the/wrong/file.feature"))`
//! would satisfy a contains-guard.
//!
//! At Milestone 1 this module is red: `feature_tracking_item` is still the
//! empty scaffold, so the exact-equality assertions fail with "no tracking
//! item is emitted". Milestone 2 supplies the real binding and these tests
//! go green without modification.

use quote::quote;

use super::feature_tracking_item;

/// The exact binding the tracking mechanism must emit, rebuilt here
/// independently of the implementation so the assertion cannot be vacuous.
fn expected_binding(rel: &str) -> String {
    let rel_lit = syn::LitStr::new(rel, proc_macro2::Span::call_site());
    let tokens = quote! {
        #[doc = "Registers the bound `.feature` file as a Cargo rebuild dependency \
                 (ADR-010). Deleting this makes `.feature`-only edits silently skip \
                 recompilation; see rstest-bdd::feature_rebuild_invalidation."]
        const _: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/", #rel_lit));
    };
    tokens.to_string()
}

#[test]
fn emits_exact_binding_for_representative_relative_path() {
    let emitted = feature_tracking_item(
        std::path::Path::new("tests/features/invalidation.feature"),
        proc_macro2::Span::call_site(),
    );
    assert_eq!(
        emitted.to_string(),
        expected_binding("tests/features/invalidation.feature"),
        "the tracking item must emit the exact deferred-path binding for a \
         relative input path"
    );
}

#[test]
fn normalizes_leading_curdir_in_relative_literal() {
    let emitted = feature_tracking_item(
        std::path::Path::new("./tests/x.feature"),
        proc_macro2::Span::call_site(),
    );
    assert_eq!(
        emitted.to_string(),
        expected_binding("tests/x.feature"),
        "a leading `./` must be stripped from the emitted relative literal"
    );
}

#[test]
fn snapshot_of_normalized_token_stream() {
    // `TokenStream::to_string()` prints no span information, so the snapshot
    // is a clean whole-text pin of the emitted form; a meaning change fails
    // loudly even where a single substring comparison could drift.
    insta::assert_snapshot!(
        feature_tracking_item(
            std::path::Path::new("tests/features/invalidation.feature"),
            proc_macro2::Span::call_site(),
        )
        .to_string()
    );
}
