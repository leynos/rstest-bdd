//! LEM-1, INV-12: insertion happens before classification.
//!
//! `absorb` takes the insertion as a required closure, so "the value was
//! offered to the context" is structural rather than a convention the driver
//! could drop. These tests pin that it is called exactly when a value came
//! back, and that the fate it returns is preserved rather than recomputed.

use rstest::rstest;

use super::fixtures::*;
use crate::runner::{
    ValueFate,
    engine::policy::{Absorbed, absorb},
};

// --- absorb: insertion happens before classification --------------------

/// A returned value is inserted through the supplied closure and its fate is
/// recorded, whatever the fate was.
///
/// One row per [`ValueFate`], named after it, so a failure says which fate was
/// lost rather than only that one was. `NoMatch` is the case that matters: it
/// is the only signal for a value that reached no later step, and the runtime
/// emits no warning for it. A run that dropped the fate here would leave a
/// renamed fixture silently green.
#[rstest]
#[case::inserted(ValueFate::Inserted)]
#[case::no_match(ValueFate::NoMatch)]
#[case::ambiguous_ignored(ValueFate::AmbiguousIgnored)]
fn a_returned_value_is_inserted_and_its_fate_kept(#[case] fate: ValueFate) {
    let absorbed = absorb(Ok(Some(Box::new(7_u32))), |value| {
        // The closure is the only place the value is visible; the driver
        // would pass `|v| ctx.insert_value(v).into()`.
        assert_eq!(value.downcast_ref::<u32>(), Some(&7));
        fate
    });

    let Absorbed {
        fate: recorded,
        error,
    } = absorbed;
    assert_eq!(recorded, Some(fate));
    assert!(error.is_none(), "a step that ran has no error");
}

/// A step that returned nothing inserts nothing, and records no fate.
///
/// `None` here is distinct from `Some(ValueFate::NoMatch)`: the former is a
/// step with no return value, the latter a value that reached nowhere. Merging
/// them would erase exactly the signal INV-12 exists to preserve.
#[test]
fn a_step_with_no_return_value_records_no_fate() {
    let mut called = false;
    let absorbed = absorb(Ok(None), |_value| {
        called = true;
        crate::runner::ValueFate::Inserted
    });

    let Absorbed { fate, error } = absorbed;
    assert!(fate.is_none());
    assert!(error.is_none());
    assert!(!called, "there was no value to insert");
}

/// A failing step's error is preserved, and no insertion happens.
///
/// What this establishes is the two things `absorb` itself decides: the error
/// survives the call, and `insert` is not invoked when there is no value. What
/// happens to that error *afterwards* is `record_step`'s business rather than
/// this function's, and nothing here asserts about it.
///
/// `expected` is taken before the call because the assertion needs a copy to
/// compare against once `absorb` has consumed its argument.
#[test]
fn a_failed_step_preserves_its_error_and_inserts_nothing() {
    let error = not_found();
    let expected = error.clone();
    let mut called = false;
    let absorbed = absorb(Err(error), |_value| {
        called = true;
        crate::runner::ValueFate::Inserted
    });

    let Absorbed { fate, error } = absorbed;
    assert!(fate.is_none(), "a failed step returns no value");
    assert!(!called, "there was no value to insert");
    assert_eq!(error, Some(expected));
}
