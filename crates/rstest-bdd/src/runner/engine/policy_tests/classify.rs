//! LEM-1: `classify` is total over the enumerated `Err` classes.
//!
//! `None` is a step that ran and every non-skip `Err` is a failure, so the
//! single discrimination is [`ExecutionError::is_skip`]. These tests walk the
//! finite domain rather than sampling it, because the domain is small enough to
//! walk and a sampled version would not notice a new class going unclassified.

use super::fixtures::*;
use crate::runner::engine::policy::{StepDecision, classify};

// --- classify: the enumerated input classes -----------------------------

#[test]
fn a_returned_value_still_continues() {
    // The value was already absorbed, so the decision sees only that the step
    // succeeded. `NoMatch` at insertion must not become a stop: that is INV-12,
    // and the decision cannot even observe the fate.
    assert!(matches!(classify(None), StepDecision::Continue));
}

#[test]
fn a_skip_is_terminal_and_carries_its_reason() {
    let decision = classify(Some(skip(Some("waiting on upstream"))));

    let StepDecision::Skip { message } = decision else {
        panic!("a skip must be terminal, got {decision:?}");
    };
    assert_eq!(message.as_deref(), Some("waiting on upstream"));
}

#[test]
fn a_skip_without_a_reason_is_still_terminal() {
    let decision = classify(Some(skip(None)));

    let StepDecision::Skip { message } = decision else {
        panic!("a bare skip must be terminal, got {decision:?}");
    };
    assert!(message.is_none());
}

/// Every non-skip `Err` class stops the run, carrying the error untouched.
///
/// The three classes are enumerated by hand rather than sampled from
/// `ExecutionError`, so this array states each *existing* class's mapping and
/// nothing more. A fourth class added to `ExecutionError` is untested here
/// until a row is added for it — no compile error forces that, and `classify`
/// itself will absorb the new variant through its own match rather than
/// failing. Exhaustiveness is `classify`'s obligation, not this test's.
#[test]
fn every_non_skip_error_is_a_failure_carrying_the_error_verbatim() {
    let cases = [not_found(), missing_fixtures(), handler_failed()];

    for error in cases {
        let expected = error.clone();
        let decision = classify(Some(error));
        let StepDecision::Fail(carried) = decision else {
            panic!("a failure must be terminal, got {decision:?}");
        };
        // Never a label and never a re-wrap: the same error, out.
        assert_eq!(carried, expected);
    }
}

/// A skip is classified as a skip and not as a failure, even though it
/// arrives as an `Err`.
///
/// This is the discrimination the whole variant list exists for: if it were
/// wrong every skipped scenario would report as failed.
#[test]
fn a_skip_is_not_classified_as_a_failure() {
    assert!(!matches!(
        classify(Some(skip(Some("Nope without a bound key")))),
        StepDecision::Fail(_),
    ));
}
