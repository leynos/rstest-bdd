//! What a caller reads when it renders a failure: `ScenarioFailure`'s own
//! `Display` and `Error` implementations.
//!
//! Split from the parent by the 400-line cap, and the split is also the
//! honest seam: the parent tests the canonical fold and the outcome's
//! accessors — values — while this file tests a projection *out* of them. The
//! failure wording is shared with `ScenarioOutcome`'s `Display` through
//! `render_with`, so a defect here is a defect in the rendered output of both,
//! and it is the file a wording change lands in.

use rstest::rstest;

use super::not_found;
use crate::runner::{ScenarioFailure, ScenarioSkip, SkipPolicyRecord, SkipRecord};

/// A forced skip at index 3: the position the renderings below must print.
///
/// `forced_failure` is `!allow_skipped && fail_on_skipped`, so setting it means
/// the two fields disagree — which is exactly the pair the runner resolves at
/// run time and the one `ScenarioFailure::ForcedSkip` exists to carry.
fn forced_skip() -> ScenarioSkip {
    ScenarioSkip::new(
        3,
        SkipRecord {
            message: None,
            source: None,
        },
        SkipPolicyRecord {
            allow_skipped: false,
            forced_failure: true,
        },
    )
}

/// `ScenarioFailure` renders as a message a caller can format.
///
/// It is the error type of the canonical fold, so it is what a caller's
/// `panic!("{failure}")` or `?` formats; without a `Display` those examples do
/// not compile, which is how this was found. The site each variant renders is
/// asserted alongside the payload because a message that lost its index or its
/// skip position would still be a `String`.
///
/// The three rows are distinct variants rather than one repeated observation:
/// each rendering is a separate arm, and a `Display` that handled `Step` and
/// fell through on the rest would pass a single-row test.
#[rstest]
#[case::step_names_its_index(
    ScenarioFailure::Step {
        index: 4,
        error: not_found(4),
    },
    "step 4",
)]
#[case::empty_plan_says_so(ScenarioFailure::EmptyPlan, "the plan contained no steps")]
fn a_failure_renders_its_variant(#[case] failure: ScenarioFailure, #[case] expected: &str) {
    let rendered = format!("{failure}");
    assert!(
        rendered.contains(expected),
        "the rendering must name what ended the run; expected {expected:?} in {rendered:?}",
    );
    assert!(
        rendered.starts_with("scenario failed"),
        "and it must read as the outcome's own failure line, so a caller can {rendered:?} \
         interchangeably with `ScenarioOutcome`'s rendering",
    );
}

/// A forced skip renders the position of the skip it was built from.
///
/// Separate from the table above because it is the one variant whose rendering
/// reads a field of a *nested* value rather than a field of the variant, and a
/// rendering that produced a plausible constant would satisfy the table.
#[test]
fn a_forced_skip_renders_its_position() {
    let rendered = format!("{}", ScenarioFailure::ForcedSkip(forced_skip()));

    assert!(
        rendered.contains("step 3"),
        "the rendering must carry the skip's own position; got {rendered:?}",
    );
    assert!(
        rendered.contains("skipped"),
        "and must say the run skipped rather than failed at a step; got {rendered:?}",
    );
}

/// The failure's `Error` impl names the step error as its source.
///
/// `source()` is what lets a caller walk the cause chain, so a failure that
/// reported `None` for a step failure would silently truncate it. The empty-plan
/// row is the control: it has no cause, and a `source()` that returned one would
/// be inventing a chain link rather than reporting the real one.
#[test]
fn a_step_failure_reports_its_error_as_the_source() {
    let failed = ScenarioFailure::Step {
        index: 0,
        error: not_found(0),
    };
    let source = std::error::Error::source(&failed).expect("a step failure has a cause");
    assert!(
        source.to_string().contains("Step not found"),
        "the cause is the step error itself; got {source}",
    );
    assert!(
        std::error::Error::source(&ScenarioFailure::EmptyPlan).is_none(),
        "an empty plan has no error to name, and must not report one",
    );
}

/// The forced-skip rendering's `Error` impl reports no source.
///
/// The third arm of `source()`, and the one the parent's control cannot reach:
/// a forced skip carries a [`ScenarioSkip`], which is not an `Error`, so a
/// `source()` that returned `Some` here would have to invent a cause — and the
/// tempting invention is the skip's message, which is prose rather than an
/// error. Asserted rather than left implicit because `_ => None` in the impl
/// covers three variants at once, and a future variant added to the `Some` arm
/// would be the way this silently changes.
#[test]
fn a_forced_skip_reports_no_error_source() {
    let forced = ScenarioFailure::ForcedSkip(forced_skip());
    assert!(
        std::error::Error::source(&forced).is_none(),
        "a skip is not an error, so it cannot be the cause of one",
    );
}
