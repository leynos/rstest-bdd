//! How one shared budget bounds a sequence of subprocesses.
//!
//! The cases are stated on elapsed budgets rather than on a stubbed clock: a
//! zero-length budget is spent the moment it is created, which reaches the
//! refusal deterministically without any sleeping, and a budget long enough not
//! to expire reaches the accept branches. `Instant` has no injectable source in
//! the standard library, and adding one here would test the stub rather than
//! the arithmetic.
//!
//! The order the cases appear in is the order a gate runs in: what is left is
//! read first, then a per-command limit is folded in, then the refusal, then
//! the accounting that stops one command spending a later one's share. The
//! production half — that the shared gate decides each probe's bound from
//! *this* budget rather than from a fresh per-command allowance — drives the
//! real gate and a real child process instead, so it lives with the gate's
//! other behavioural cases in `preparation::tests::deadline_gate`.

use std::{io, time::Duration};

use super::Deadline;

/// A budget that has already run out.
///
/// The expiry is `Instant::now()`, so by the time the case reads it the
/// remaining time is zero or negative. `saturating_duration_since` folds both
/// into zero, which is the state [`Deadline::remaining`] reports as spent.
fn spent() -> Deadline { Deadline::starting_now(Duration::ZERO) }

/// A budget far longer than any test needs, so it cannot expire mid-case.
fn ample() -> Deadline { Deadline::starting_now(Duration::from_secs(3600)) }

#[test]
fn a_spent_budget_reports_nothing_remaining() {
    assert_eq!(
        spent().remaining(),
        None,
        "a zero-length budget must report itself spent rather than a zero remainder"
    );
}

#[test]
fn an_ample_budget_reports_what_is_left() {
    let budget = Duration::from_secs(3600);
    let left = Deadline::starting_now(budget)
        .remaining()
        .expect("an hour-long budget is not spent");
    assert!(
        left <= budget,
        "the remainder cannot exceed the budget it started from: {left:?}"
    );
    assert!(
        left > Duration::from_secs(3500),
        "the remainder must be most of a budget that was just created: {left:?}"
    );
}

#[test]
fn the_cap_is_the_smaller_of_the_remainder_and_the_limit() {
    let deadline = ample();
    // A limit below the remainder is taken as-is; a limit above it is reduced
    // to the remainder. Both arms are exercised so a `cap` that always returned
    // one of them would fail here.
    let tight = Duration::from_secs(1);
    assert_eq!(
        deadline.cap(tight, "a probe").expect("a live budget"),
        tight,
        "a per-command limit below the remainder must be honoured"
    );
    let budget = Duration::from_secs(30);
    let reduced = Deadline::starting_now(budget)
        .cap(Duration::from_secs(600), "a probe")
        .expect("a live budget");
    // Compared against the budget rather than against a second `remaining()`
    // read: the clock advances between the two, so exact equality with an
    // earlier reading is a property of the test, not of the cap.
    assert!(
        reduced <= budget,
        "a limit above the remainder must be reduced to the budget: {reduced:?} vs {budget:?}"
    );
    assert!(
        reduced > Duration::from_secs(25),
        "the reduction must keep most of a live budget: {reduced:?}"
    );
}

#[test]
fn a_spent_budget_refuses_to_start_a_command() {
    let error = spent()
        .cap(Duration::from_secs(60), "`rustc -vV`")
        .expect_err("a spent budget must not hand out a bound");
    assert_eq!(
        error.kind(),
        io::ErrorKind::TimedOut,
        "a refusal is a timeout, not a spawn failure: {error}"
    );
    let text = error.to_string();
    assert!(
        text.contains("`rustc -vV`"),
        "the refusal must name the command it declined to start: {text}"
    );
}

#[test]
fn no_single_command_may_be_given_more_than_the_whole_budget() {
    // The property the type exists for: a generous per-command limit cannot
    // buy a command more time than the run has left. Without the budget folded
    // in, a 600 s probe limit would admit a 600 s probe under a 30 s run.
    let budget = Duration::from_secs(30);
    let whole = Deadline::starting_now(budget)
        .cap(Duration::from_secs(600), "a probe")
        .expect("a live budget");
    assert!(
        whole <= budget,
        "one command may not be given more than the whole budget: {whole:?} vs {budget:?}"
    );
    // And a limit below what is left is not inflated to it.
    let tight = Deadline::starting_now(budget)
        .cap(Duration::from_secs(2), "a probe")
        .expect("a live budget");
    assert_eq!(
        tight,
        Duration::from_secs(2),
        "a per-command limit below the remainder must be taken as-is"
    );
}

#[test]
fn a_short_budget_stays_live_for_a_second_command() {
    // Two commands against one budget are each bounded by what is left, so the
    // second is not given the first one's allowance as well. This is the
    // accounting the deadline was introduced for; measuring it without a clock
    // stub means asserting on the bound rather than on elapsed time.
    let budget = Duration::from_secs(100);
    let deadline = Deadline::starting_now(budget);
    let first = deadline
        .cap(Duration::from_secs(60), "first")
        .expect("a live budget");
    let second = deadline
        .cap(Duration::from_secs(60), "second")
        .expect("a live budget");
    assert!(
        first <= budget && second <= budget,
        "neither command may exceed the shared budget: {first:?}, {second:?} vs {budget:?}"
    );
}

#[test]
fn a_later_command_is_charged_against_what_is_left_not_its_own_limit() {
    // The slow-success path CodeRabbit named, stated on the accounting rather
    // than on a clock: a first command may take the whole budget, and the
    // second must then be refused instead of being given its own fresh
    // allowance. Without this, a query that succeeded at 119 s of a 120 s
    // budget left two 60 s probes free to run past the runner's period.
    let budget = Duration::from_secs(1);
    let deadline = Deadline::starting_now(budget);
    // Spend the budget the way a slow query would: wait it out.
    std::thread::sleep(budget + Duration::from_millis(50));
    let error = deadline
        .cap(Duration::from_secs(60), "`cargo -vV`")
        .expect_err("a budget spent by the first command must refuse the second");
    assert_eq!(
        error.kind(),
        io::ErrorKind::TimedOut,
        "the second command is refused by the deadline, not by its own limit: {error}"
    );
    assert!(
        error.to_string().contains("`cargo -vV`"),
        "the refusal must name the probe that never started: {error}"
    );
}
