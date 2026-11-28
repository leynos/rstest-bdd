//! Pinning tests for the shared step-to-token builder.
//!
//! `process_steps` (regular scenarios) and `process_steps_substituted`
//! (scenario outlines) must agree on the token shape of a step's value,
//! docstring, and data table. These tests fix that shape so the two paths
//! cannot drift while the literal construction is shared.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

use super::{
    ExampleHeaders,
    ExampleRow,
    ParsedStep,
    blank,
    process_steps,
    process_steps_substituted,
};

/// Build a step with the supplied content, keeping the shared blank defaults.
fn step_with_content(
    text: &str,
    docstring: Option<&str>,
    table: Option<Vec<Vec<String>>>,
) -> ParsedStep {
    ParsedStep {
        text: text.to_owned(),
        docstring: docstring.map(str::to_owned),
        table,
        ..blank()
    }
}

/// Compare two token streams through the same deterministic rendering.
fn assert_tokens(actual: &TokenStream2, expected: &TokenStream2, context: &str) {
    assert_eq!(
        actual.to_string(),
        expected.to_string(),
        "unexpected token shape: {context}"
    );
}

#[test]
fn substituted_step_populates_value_docstring_and_table_tokens() {
    let step = step_with_content(
        "I have <count> <item>",
        Some("subtotal: <count>"),
        Some(vec![vec!["<item>".to_owned(), "<count>".to_owned()]]),
    );
    let headers = ExampleHeaders::new(vec!["count".to_owned(), "item".to_owned()]);
    let row = ExampleRow::new(vec!["5".to_owned(), "apples".to_owned()]);

    let (keywords, values, docstrings, tables) =
        process_steps_substituted(&[step], &headers, &row).expect("substitution should succeed");

    assert_eq!(keywords.len(), 1, "a one-step scenario yields one keyword");
    let value = values.first().expect("one value token for one step");
    let docstring = docstrings
        .first()
        .expect("one docstring token for one step");
    let table = tables.first().expect("one table token for one step");
    assert_tokens(
        value,
        &quote! { "I have 5 apples" },
        "value tokens carry the substituted step text",
    );
    assert_tokens(
        docstring,
        &quote! { Some("subtotal: 5") },
        "docstring tokens carry the substituted docstring",
    );
    assert_tokens(
        table,
        &quote! { Some(&[&["apples", "5"][..]][..]) },
        "table tokens carry the substituted cells",
    );
}

#[test]
fn placeholder_free_steps_share_tokens_across_both_processors() {
    let step = step_with_content(
        "I have apples",
        Some("note: apples"),
        Some(vec![vec!["apples".to_owned(), "5".to_owned()]]),
    );
    let headers = ExampleHeaders::new(vec!["count".to_owned(), "item".to_owned()]);
    let row = ExampleRow::new(vec!["5".to_owned(), "apples".to_owned()]);

    let (plain_keywords, plain_values, plain_docstrings, plain_tables) =
        process_steps(std::slice::from_ref(&step));
    let (sub_keywords, sub_values, sub_docstrings, sub_tables) =
        process_steps_substituted(&[step], &headers, &row).expect("substitution should succeed");

    assert_eq!(
        plain_keywords.len(),
        sub_keywords.len(),
        "both paths emit one keyword per step"
    );
    assert_eq!(
        plain_keywords
            .iter()
            .map(TokenStream2::to_string)
            .collect::<Vec<_>>(),
        sub_keywords
            .iter()
            .map(TokenStream2::to_string)
            .collect::<Vec<_>>(),
        "a placeholder-free step keeps its keyword through substitution"
    );
    assert_eq!(
        plain_values
            .iter()
            .map(TokenStream2::to_string)
            .collect::<Vec<_>>(),
        sub_values
            .iter()
            .map(TokenStream2::to_string)
            .collect::<Vec<_>>(),
        "a placeholder-free step keeps its value tokens through substitution"
    );
    assert_eq!(
        plain_docstrings
            .iter()
            .map(TokenStream2::to_string)
            .collect::<Vec<_>>(),
        sub_docstrings
            .iter()
            .map(TokenStream2::to_string)
            .collect::<Vec<_>>(),
        "a placeholder-free step keeps its docstring tokens through substitution"
    );
    assert_eq!(
        plain_tables
            .iter()
            .map(TokenStream2::to_string)
            .collect::<Vec<_>>(),
        sub_tables
            .iter()
            .map(TokenStream2::to_string)
            .collect::<Vec<_>>(),
        "a placeholder-free step keeps its table tokens through substitution"
    );
}

#[test]
fn unknown_placeholder_surfaces_compile_error_tokens() {
    let step = step_with_content("I have <missing>", None, None);
    let headers = ExampleHeaders::new(vec!["count".to_owned()]);
    let row = ExampleRow::new(vec!["5".to_owned()]);

    let Err(tokens) = process_steps_substituted(&[step], &headers, &row) else {
        panic!("an unknown placeholder must fail substitution");
    };

    let rendered = tokens.to_string();
    assert!(
        rendered.contains("Placeholder '<missing>' not found in Examples table"),
        "compile error should name the missing placeholder: {rendered}"
    );
    assert!(
        rendered.contains("compile_error"),
        "substitution failure should render a compile error: {rendered}"
    );
}
