//! Verification for the pure policy-diagnostics kernel.
//!
//! The kernel's domain is finite, so these tests enumerate it rather than
//! sample it. Each milestone adds its own dimensions together with the
//! enumerator's asserted state count (decision D19), which keeps a field from
//! being introduced without a property that reads it.

mod classify_props;
mod convergence;
mod enumerator;
mod messages;
mod wording;
