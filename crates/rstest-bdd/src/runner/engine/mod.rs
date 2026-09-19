//! The scenario engine, split into policy and two drivers.
//!
//! [`policy`] holds every decision a run makes and does no I/O: it classifies
//! a step's result and assembles the terminal outcome. `drive_sync` and
//! `drive_async` hold the loops, and they differ only in whether a step is
//! awaited. A driver resolves, executes, records, and delegates; it contains
//! no `if` on a step result, because the stop decision reaches it as a
//! [`policy::StepDecision`] it matches once.
//!
//! The split is not cosmetic. Rust cannot express one loop that is both
//! synchronous and asynchronous without macro duplication or per-step boxing,
//! so there are necessarily two loops; putting the decision in neither is what
//! keeps them from disagreeing. The whole of the decision logic is then
//! checkable with hand-built values, which is why `policy` has no
//! `StepContext`, no registry, and no `async` in any signature.
//!
//! This module is private to `runner`, and `classify`, `assemble`,
//! `StepDecision`, `Terminal`, `SkipPolicy`, and `Absorbed` are `pub(crate)`:
//! they are this milestone's internal decomposition of one public entry point,
//! not adopted API. Publishing them would freeze a shape that exists to be
//! refactored behind `run_scenario`.

pub(super) mod drive_sync;
pub(crate) mod policy;

#[cfg(test)]
mod policy_tests;
