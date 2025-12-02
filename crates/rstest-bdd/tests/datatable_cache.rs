//! Behavioural tests for cached data table conversion.

use rstest_bdd::datatable::CachedTable;
use rstest_bdd::{lookup_step, StepContext, StepKeyword};
use rstest_bdd_macros::given;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::thread;

fn cached_calls() -> &'static Mutex<HashMap<thread::ThreadId, Vec<usize>>> {
    static CALLS: OnceLock<Mutex<HashMap<thread::ThreadId, Vec<usize>>>> = OnceLock::new();
    CALLS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cached_values() -> &'static Mutex<HashMap<thread::ThreadId, Vec<String>>> {
    static VALUES: OnceLock<Mutex<HashMap<thread::ThreadId, Vec<String>>>> = OnceLock::new();
    VALUES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn record_call(ptr: usize) {
    let mut calls = cached_calls()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    calls.entry(thread::current().id()).or_default().push(ptr);
}

fn record_value(value: String) {
    let mut values = cached_values()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    values
        .entry(thread::current().id())
        .or_default()
        .push(value);
}

fn take_calls() -> Vec<usize> {
    cached_calls()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(&thread::current().id())
        .unwrap_or_default()
}

fn take_values() -> Vec<String> {
    cached_values()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(&thread::current().id())
        .unwrap_or_default()
}

static CONVERSIONS: AtomicUsize = AtomicUsize::new(0);

fn reset_conversions() {
    CONVERSIONS.store(0, Ordering::Relaxed);
}

fn conversion_count() -> usize {
    CONVERSIONS.load(Ordering::Relaxed)
}

#[given("a cached table:")]
fn cached_table(datatable: CachedTable) {
    let ptr = datatable.as_ptr() as usize;
    record_call(ptr);
}

#[given("another cached table:")]
fn another_cached_table(datatable: CachedTable) {
    let ptr = datatable.as_ptr() as usize;
    record_call(ptr);
}

#[derive(Debug)]
struct CountingTable(Vec<Vec<String>>);

impl TryFrom<Vec<Vec<String>>> for CountingTable {
    type Error = String;

    fn try_from(value: Vec<Vec<String>>) -> Result<Self, Self::Error> {
        CONVERSIONS.fetch_add(1, Ordering::Relaxed);
        Ok(Self(value))
    }
}

impl CountingTable {}

#[given("a counting table:")]
fn counting_table(#[datatable] mut datatable: CountingTable) {
    if let Some(first) = datatable.0.first().and_then(|row| row.first()).cloned() {
        record_value(first);
    }

    if let Some(first_mut) = datatable.0.first_mut().and_then(|row| row.first_mut()) {
        first_mut.push_str(" mutated");
    }
}

#[test]
fn cached_table_reuses_conversion_for_identical_table_pointer() {
    const TABLE: &[&[&str]] = &[&["foo", "bar"], &["baz", "qux"]];

    take_calls();

    let step_fn = lookup_step(StepKeyword::Given, "a cached table:".into())
        .unwrap_or_else(|| panic!("cached table step should be registered"));
    let mut ctx = StepContext::default();

    for _ in 0..2 {
        let _exec = step_fn(&mut ctx, "a cached table:", None, Some(TABLE))
            .unwrap_or_else(|err| panic!("cached step should succeed: {err}"));
    }

    let calls = take_calls();
    let (Some(first), Some(second)) = (calls.first(), calls.get(1)) else {
        panic!("expected two cached table entries: {calls:?}");
    };
    assert_eq!(first, second, "cached table should reuse shared data");
}

#[test]
fn cached_table_cache_separates_distinct_tables() {
    const TABLE_ONE: &[&[&str]] = &[&["alpha"], &["beta"]];
    const TABLE_TWO: &[&[&str]] = &[&["gamma"], &["delta"]];

    take_calls();

    let step_fn = lookup_step(StepKeyword::Given, "a cached table:".into())
        .unwrap_or_else(|| panic!("cached table step should be registered"));
    let mut ctx = StepContext::default();

    let _ = step_fn(&mut ctx, "a cached table:", None, Some(TABLE_ONE))
        .unwrap_or_else(|err| panic!("first cached table should succeed: {err}"));
    let _ = step_fn(&mut ctx, "a cached table:", None, Some(TABLE_TWO))
        .unwrap_or_else(|err| panic!("second cached table should succeed: {err}"));

    let calls = take_calls();
    let (Some(first), Some(second)) = (calls.first(), calls.get(1)) else {
        panic!("expected two cached table entries: {calls:?}");
    };
    assert_ne!(
        first, second,
        "distinct tables must not share cached conversions"
    );
}

#[test]
fn cached_table_cache_is_scoped_per_step_wrapper() {
    const TABLE: &[&[&str]] = &[&["foo", "bar"], &["baz", "qux"]];

    take_calls();

    let first_step_fn = lookup_step(StepKeyword::Given, "a cached table:".into())
        .unwrap_or_else(|| panic!("cached table step should be registered"));
    let second_step_fn = lookup_step(StepKeyword::Given, "another cached table:".into())
        .unwrap_or_else(|| panic!("another cached table step should be registered"));
    let mut ctx = StepContext::default();

    let _ = first_step_fn(&mut ctx, "a cached table:", None, Some(TABLE))
        .unwrap_or_else(|err| panic!("first cached table should succeed: {err}"));
    let calls_first = take_calls();
    let first_ptr = calls_first
        .first()
        .copied()
        .unwrap_or_else(|| panic!("expected one call from first step, got {calls_first:?}"));

    let _ = second_step_fn(&mut ctx, "another cached table:", None, Some(TABLE))
        .unwrap_or_else(|err| panic!("second cached table should succeed: {err}"));
    let calls_second = take_calls();
    let second_ptr = calls_second
        .first()
        .copied()
        .unwrap_or_else(|| panic!("expected one call from second step, got {calls_second:?}"));

    assert_ne!(
        first_ptr, second_ptr,
        "cached Arc pointers should differ between step wrappers, indicating separate caches",
    );
}

#[test]
fn datatable_vec_path_reuses_cache_and_clones_per_call() {
    const TABLE: &[&[&str]] = &[&["foo", "bar"], &["baz", "qux"]];

    take_calls();
    take_values();
    reset_conversions();

    let step_fn = lookup_step(StepKeyword::Given, "a counting table:".into())
        .unwrap_or_else(|| panic!("counting table step should be registered"));
    let mut ctx = StepContext::default();

    for _ in 0..2 {
        let _ = step_fn(&mut ctx, "a counting table:", None, Some(TABLE))
            .unwrap_or_else(|err| panic!("counting table step should succeed: {err}"));
    }

    assert_eq!(
        conversion_count(),
        2,
        "Vec conversion should occur per call"
    );

    let values = take_values();
    let [first, second] = values.as_slice() else {
        panic!("expected two captured values, got {values:?}");
    };
    assert_eq!(first, "foo", "first call should start with original value");
    assert_eq!(
        second, "foo",
        "second call should not observe mutations from the first call",
    );
}
