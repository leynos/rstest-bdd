//! Behavioural tests for cached data table conversion.

use rstest_bdd::datatable::CachedTable;
use rstest_bdd::{lookup_step, StepContext, StepKeyword};
use rstest_bdd_macros::given;
use std::sync::{Mutex, OnceLock};

fn cached_calls() -> &'static Mutex<Vec<usize>> {
    static CALLS: OnceLock<Mutex<Vec<usize>>> = OnceLock::new();
    CALLS.get_or_init(|| Mutex::new(Vec::new()))
}

#[given("a cached table:")]
fn cached_table(datatable: CachedTable) {
    let ptr = std::sync::Arc::as_ptr(&datatable.as_arc()) as usize;
    let mut calls = cached_calls()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    calls.push(ptr);
}

#[test]
fn cached_table_reuses_conversion_for_identical_table_pointer() {
    const TABLE: &[&[&str]] = &[&["foo", "bar"], &["baz", "qux"]];

    cached_calls()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clear();

    let step_fn = lookup_step(StepKeyword::Given, "a cached table:".into())
        .unwrap_or_else(|| panic!("cached table step should be registered"));
    let mut ctx = StepContext::default();

    for _ in 0..2 {
        let _exec = step_fn(&mut ctx, "a cached table:", None, Some(TABLE))
            .unwrap_or_else(|err| panic!("cached step should succeed: {err}"));
    }

    let calls = cached_calls()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (Some(first), Some(second)) = (calls.first(), calls.get(1)) else {
        panic!("expected two cached table entries: {calls:?}");
    };
    assert_eq!(first, second, "cached table should reuse shared data");
}

#[test]
fn cached_table_cache_separates_distinct_tables() {
    const TABLE_ONE: &[&[&str]] = &[&["alpha"], &["beta"]];
    const TABLE_TWO: &[&[&str]] = &[&["gamma"], &["delta"]];

    cached_calls()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clear();

    let step_fn = lookup_step(StepKeyword::Given, "a cached table:".into())
        .unwrap_or_else(|| panic!("cached table step should be registered"));
    let mut ctx = StepContext::default();

    let _ = step_fn(&mut ctx, "a cached table:", None, Some(TABLE_ONE))
        .unwrap_or_else(|err| panic!("first cached table should succeed: {err}"));
    let _ = step_fn(&mut ctx, "a cached table:", None, Some(TABLE_TWO))
        .unwrap_or_else(|err| panic!("second cached table should succeed: {err}"));

    let calls = cached_calls()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (Some(first), Some(second)) = (calls.first(), calls.get(1)) else {
        panic!("expected two cached table entries: {calls:?}");
    };
    assert_ne!(
        first, second,
        "distinct tables must not share cached conversions"
    );
}
