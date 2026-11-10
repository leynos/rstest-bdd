//! Tests for the data-table literals emitted into generated step tables.
//!
//! Outline steps live in a `const`, so the literals must avoid range
//! indexing (`[..]`), which calls the non-const `Index` impl.

use rstest::rstest;

use crate::codegen::scenario::helpers::generate_table_tokens;

fn table(rows: &[&[&str]]) -> Vec<Vec<String>> {
    rows.iter()
        .map(|row| row.iter().map(|&cell| cell.to_owned()).collect())
        .collect()
}

#[rstest]
#[case::no_table(None, "None")]
#[case::empty_table(Some(table(&[])), "Some (& [] as & [& [& str]])")]
#[case::rows(
    Some(table(&[&["a", "b"], &["c", "d"]])),
    concat!(
        "Some (& [& [\"a\" , \"b\"] as & [& str] , ",
        "& [\"c\" , \"d\"] as & [& str]] as & [& [& str]])",
    )
)]
fn tables_render_as_const_safe_coercions(
    #[case] rows: Option<Vec<Vec<String>>>,
    #[case] expected: &str,
) {
    let rendered = generate_table_tokens(rows.as_deref()).to_string();
    assert_eq!(rendered, expected);
    assert!(!rendered.contains(".."), "range indexing in {rendered}");
}
