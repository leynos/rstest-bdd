//! The scenario engine, split into policy, the shared step handling, and two
//! drivers.
//!
//! [`policy`] holds every decision a run makes and does no I/O: it classifies
//! a step's result and assembles the terminal outcome. [`drive`] holds what
//! both drivers would otherwise decide twice — the request view and the
//! step-result record. `drive_sync` and `drive_async` are then each a `for`
//! loop, an executor call, and a `break`, differing only in whether the call is
//! awaited.
//!
//! A driver contains no `if` on a step result, because the stop decision
//! reaches it as a [`policy::Terminal`] it hands straight back to `assemble`.
//!
//! The split is not cosmetic, and it is not the only one available. One shared
//! loop is expressible — write it against a boxed next-poll future and give
//! each driver a poller — and D23 records why it was rejected: the synchronous
//! entry point is a plain `fn`, so sharing the loop would make it depend on a
//! poller that never yields, with nothing in the type system to enforce it.
//! Sharing the *decisions* gets the same protection against drift without that
//! assumption, because a decision that is not written twice cannot disagree
//! with itself.
//!
//! The whole of the decision logic is checkable with hand-built values, which
//! is why `policy` has no `StepContext`, no registry, and no `async` in any
//! signature.
//!
//! This module is private to `runner`, and `classify`, `assemble`,
//! `StepDecision`, `Terminal`, `SkipPolicy`, and `Absorbed` are `pub(crate)`:
//! they are this milestone's internal decomposition of one public entry point,
//! not adopted API. Publishing them would freeze a shape that exists to be
//! refactored behind `run_scenario`.

pub(super) mod drive;
pub(super) mod drive_async;
pub(super) mod drive_sync;
pub(crate) mod policy;

#[cfg(test)]
mod policy_tests;
