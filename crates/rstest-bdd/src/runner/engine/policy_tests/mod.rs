//! LEM-1: the decision layer is pure, total, and enumerable.
//!
//! Every test here drives `classify` or `assemble` directly, with no registry
//! and no `StepContext`. That is the point of the split: the stop decision is
//! single-sourced, so the two drivers cannot disagree about it, and the whole
//! of it is checkable without running a scenario.
//!
//! The domain is finite and small. `execute_step` returns
//! `Result<Option<Box<dyn Any>>, ExecutionError>`, and for decision purposes
//! the `Err` side collapses to the four classes the fixtures build: `Skip`,
//! `StepNotFound`, `MissingFixtures`, and `HandlerFailed`. `FailureKind` is
//! deliberately not part of the decision, so `HandlerFailed`'s sub-kinds are
//! not distinct inputs here.
//!
//! # What these tests can and cannot establish
//!
//! They establish that `classify` is total over the enumerated classes and
//! that each class maps to the decision D18 records. They establish nothing
//! about whether the drivers *call* it correctly — that is INV-1's and INV-5's
//! job, discharged by the sequence property tests. Keeping the two apart is
//! deliberate: a test driving a real scenario could fail for a driver bug and
//! be misread as a `classify` bug.
//!
//! # Splitting the fixtures out
//!
//! `fixtures` is a sibling module rather than a `#[cfg(test)]` block inside
//! each file because the three test modules would otherwise each carry their
//! own copy, and the module line cap (400 lines) is what forced the question.
//! The seam is real: the fixtures describe *inputs*, and each test module
//! asserts a different property over them.

mod absorb;
mod assemble;
mod classify;
mod fixtures;
