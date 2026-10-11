//! The four sequence invariants, each as a property over generated plans.
//!
//! Every test here that can fold a case in does so, and asserts *afterwards*
//! that the classes its invariant names were reached. The reasoning is in the
//! crate root's module documentation; what matters here is that the per-case
//! `prop_assert!` only ever reports a *violation*, and the post-run assertion
//! is what reports a generator that stopped producing cases capable of
//! violating anything.
//!
//! [`every_invocation_is_recorded_in_plan_order`] is the exception: INV-2 needs
//! no non-vacuity accumulator, because every plan it is handed exercises it —
//! there is no shape of plan for which "one record per invocation" is trivially
//! true. Record counts are decided by `steps.len()` rather than by a class the
//! generator might stop drawing.

use proptest::prelude::*;
use rstest_bdd::runner::StepStatus;

use super::{
    check,
    sequence::{Witnesses, run_case},
};

/// INV-1: no invocation past the terminal index is executed.
#[test]
fn no_invocation_past_the_terminal_index_is_executed() {
    let mut witnesses = Witnesses::default();

    check(|steps, arrangement| {
        let run = run_case(&steps, arrangement);
        witnesses.record(&steps, arrangement, &run);

        prop_assert!(
            !run.terminal_exceeded(),
            "invocation {:?} ran past the terminal index {:?}; {}",
            run.first_exceeding(),
            run.terminal_index(),
            run.describe(&steps)
        );
        Ok(())
    });

    witnesses.assert_complete();
}

/// INV-2: every invocation is recorded, in plan order, with its own identity.
#[test]
fn every_invocation_is_recorded_in_plan_order() {
    check(|steps, arrangement| {
        let run = run_case(&steps, arrangement);
        prop_assert_eq!(
            run.outcome.steps().len(),
            steps.len(),
            "one record per invocation; {}",
            run.describe(&steps)
        );

        for (index, (record, step)) in run.outcome.steps().iter().zip(&steps).enumerate() {
            prop_assert_eq!(record.index(), index, "record {} is misindexed", index);
            prop_assert_eq!(
                record.keyword(),
                step.keyword,
                "record {} must carry its own keyword",
                index
            );
            prop_assert_eq!(
                record.text(),
                step.kind.text(index),
                "record {} must carry its own text",
                index
            );
            prop_assert_eq!(
                record
                    .source()
                    .map(rstest_bdd::runner::SourceLocation::line),
                Some(step.line),
                "record {} must carry its own source line",
                index
            );
        }

        let terminal = run.terminal_index();
        for (index, record) in run.outcome.steps().iter().enumerate() {
            if terminal.is_some_and(|terminal| index > terminal) {
                prop_assert_eq!(
                    record.status(),
                    StepStatus::Bypassed,
                    "record {} follows terminal {:?} and must be bypassed; {}",
                    index,
                    terminal,
                    run.describe(&steps)
                );
            }
        }
        Ok(())
    });
}

/// INV-3: a producer's value reaches every later observer and no earlier one.
#[test]
fn a_returned_value_is_visible_only_after_its_producer() {
    let mut witnesses = Witnesses::default();

    check(|steps, arrangement| {
        let run = run_case(&steps, arrangement);
        witnesses.record(&steps, arrangement, &run);

        prop_assert!(
            run.visibility_violation().is_none(),
            "{:?}; {}",
            run.visibility_violation(),
            run.describe(&steps)
        );
        Ok(())
    });

    witnesses.assert_visibility_complete();
}

/// INV-12: every value-returning invocation records its fate, all three occur.
#[test]
fn every_returned_value_records_its_fate() {
    let mut witnesses = Witnesses::default();

    check(|steps, arrangement| {
        let run = run_case(&steps, arrangement);
        witnesses.record(&steps, arrangement, &run);

        for (index, fate) in run.fates(&steps) {
            let record = run.outcome.steps().get(index);
            let passed = record.is_some_and(|record| record.status() == StepStatus::Passed);
            // The kind is read from the same lookup as the status, so a plan
            // whose record count and kind list disagreed cannot have this
            // assertion name a kind for an invocation the run never recorded.
            let kind = steps.get(index).map(|step| step.kind);
            prop_assert_eq!(
                fate.is_some(),
                passed,
                "invocation {} (kind {:?}) {} and recorded {:?}; {}",
                index,
                kind,
                if passed { "passed" } else { "did not pass" },
                fate,
                run.describe(&steps)
            );
        }
        Ok(())
    });

    witnesses.assert_fates_complete();
}
