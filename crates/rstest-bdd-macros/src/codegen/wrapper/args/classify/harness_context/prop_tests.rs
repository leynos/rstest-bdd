//! Property tests for the `#[harness_context]` classifier.
//!
//! The unit tests pin specific parameters and exact diagnostics; these cover
//! the same mapping across generated identifiers and request spellings. The
//! property asserted here: for any valid, non-reserved parameter identifier,
//! every request spelling classifies the parameter as an `Arg::Fixture` named
//! `rstest_bdd_harness_context`, consuming no placeholder.

use super::super::super::extract_args;
use super::super::super::Arg;
use googletest::prelude::*;
use proptest::prelude::*;
use std::collections::HashSet;

/// First character of a generated identifier: always a letter, so the base is
/// never empty and never starts with a digit or underscore.
const HEAD_CHARS: [char; 5] = ['a', 'b', 'm', 'q', 'z'];

/// Subsequent characters, chosen so no combination spells a Rust keyword;
/// the filter below is a safety net rather than the primary defence.
const TAIL_CHARS: [char; 10] = ['a', 'b', 'c', 'x', 'y', 'z', '0', '1', '9', '_'];

/// Strategy yielding a valid, non-keyword Rust identifier that is not one of
/// the names the pipeline reserves (`datatable`, `docstring`).
fn param_ident() -> impl Strategy<Value = String> {
    (
        prop::sample::select(HEAD_CHARS.as_slice()),
        prop::collection::vec(prop::sample::select(TAIL_CHARS.as_slice()), 0..6),
    )
        .prop_map(|(head, tail)| {
            let mut base = String::from(head);
            base.extend(tail);
            base
        })
        .prop_filter("must parse as a non-keyword, non-reserved identifier", |name| {
            syn::parse_str::<syn::Ident>(name).is_ok() && name != "datatable" && name != "docstring"
        })
}

/// The three request spellings a step may use for the harness context.
#[derive(Debug, Clone)]
enum Spelling {
    /// `#[harness_context] name: &C`
    Marker,
    /// `#[from(rstest_bdd_harness_context)] name: &C`
    From,
    /// `name: &C` where the parameter is literally named after the reserved
    /// fixture key
    ReservedName,
}

impl Spelling {
    /// Render this spelling for a parameter named `param`.
    ///
    /// The reserved-name spelling ignores `param` because it only exists when
    /// the parameter literally carries the reserved key as its identifier.
    fn render(&self, param: &str) -> String {
        match self {
            Self::Marker => format!("fn s(#[harness_context] {param}: &C) {{}}"),
            Self::From => {
                format!("fn s(#[from(rstest_bdd_harness_context)] {param}: &C) {{}}")
            }
            Self::ReservedName => {
                format!("fn s(rstest_bdd_harness_context: &C) {{}}")
            }
        }
    }
}

/// Strategy yielding each request spelling.
fn spelling() -> impl Strategy<Value = Spelling> {
    prop_oneof![
        Just(Spelling::Marker),
        Just(Spelling::From),
        Just(Spelling::ReservedName),
    ]
}

proptest! {
    /// Every request spelling binds the reserved fixture key for any
    /// permissible parameter identifier.
    #[test]
    fn every_spelling_binds_the_reserved_key(param in param_ident(), spelling in spelling()) {
        let mut func: syn::ItemFn = syn::parse_str(&spelling.render(&param))
            .map_err(|e| TestCaseError::fail(e.to_string()))?;
        let mut placeholders = HashSet::new();
        let extracted = extract_args(&mut func, &mut placeholders)
            .map_err(|e| TestCaseError::fail(e.to_string()))?;

        verify_that!(
            extracted.args,
            elements_are![matches_pattern!(Arg::Fixture {
                name: displays_as(eq("rstest_bdd_harness_context")),
                ..
            })]
        )?;
        prop_assert!(placeholders.is_empty());
    }
}
