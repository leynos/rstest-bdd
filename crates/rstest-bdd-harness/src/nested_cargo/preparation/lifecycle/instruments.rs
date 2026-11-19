//! The two boundaries a caller may substitute for the real ones.
//!
//! The probe resolves the compiler's identity and the reporter carries the
//! gate's diagnostics. They travel together in [`Instruments`] because every
//! caller that overrides one of them overrides the other at the same time, and
//! threading them separately made the innermost boundary take five arguments.
//!
//! The production bindings live here too, beside the type that groups them:
//! [`instruments_of`] is the one place the real probe and the real reporter are
//! paired, so the compiler the gate identifies and the stream it writes through
//! cannot drift apart.

use std::io;

use super::super::{Preparation, toolchain::toolchain_identity};

/// The two boundaries a caller may substitute for the real ones.
///
/// The probe resolves the compiler's identity and the reporter carries this
/// module's diagnostics. They travel together because they are overridden for
/// the same purpose and at the same time: a test that cannot make `cargo -vV`
/// fail and cannot read stderr needs both injected, and a caller that supplies
/// one of them wants the other's production binding. Grouping them also keeps
/// the innermost gate inside the argument budget this repository enforces.
///
/// The asynchronous forms are the ones the gate takes: the probe is called
/// exactly once, and its error stops the gate before any stamp is read.
pub struct Instruments<P, R> {
    /// Resolves the toolchain identity the fingerprint is built from.
    pub probe: P,
    /// Receives every diagnostic the gate emits, as a finished message.
    pub report: R,
}

impl<P, R> Instruments<P, R> {
    /// Pair a probe with a reporter.
    ///
    /// # Parameters
    /// - `probe`: The toolchain probe; called once per gate.
    /// - `report`: Where diagnostics go; called at most twice per gate, once before the lock is
    ///   taken and once after.
    ///
    /// # Returns
    /// The two grouped, in the field order the gate takes them.
    #[must_use]
    pub fn new(probe: P, report: R) -> Self { Self { probe, report } }
}

/// Report a preparation diagnostic the way production does.
///
/// Resolved as a function value rather than written inline at each call, so
/// [`instruments_of`], `invalidate`'s and the production entry points share one
/// binding: a second spelling of [`report_stderr`] would let the two diverge
/// without any test noticing. Shared with the sibling module rather than
/// duplicated there for that reason.
pub(in crate::nested_cargo::preparation) fn reporter_of() -> impl FnMut(&str) { report_stderr }

/// Write one preparation diagnostic to standard error.
///
/// Preparation is intentionally visible in test logs: its elapsed time and
/// compiled-unit count are the evidence that the cold work left the timed
/// assertions.
///
/// It lives beside the reporter that production binds to it rather than in the
/// stamp protocol: what a person is told about a refusal is the gate's
/// decision, and the stamp module has no way to observe stderr to test it
/// against.
fn report_stderr(message: &str) {
    use std::io::Write as _;
    let _ = writeln!(io::stderr(), "nested preparation: {message}");
}

/// Bind the real toolchain probe to the preparation's own environment.
///
/// Every entry point that supplies the real probe needs exactly this binding,
/// and it must be the preparation's environment rather than the process's: the
/// compiler identified has to be the one the nested build will run.
fn probe_of(preparation: &Preparation) -> impl FnOnce() -> io::Result<String> + '_ {
    move || toolchain_identity(&preparation.environment, &preparation.working_directory)
}

/// The real boundaries, bound to `preparation`.
///
/// One binding, used by both production entry points, so the compiler the gate
/// identifies and the reporter it writes through cannot drift apart.
pub(in crate::nested_cargo::preparation) fn instruments_of(
    preparation: &Preparation,
) -> Instruments<impl FnOnce() -> io::Result<String> + '_, impl FnMut(&str)> {
    Instruments::new(probe_of(preparation), reporter_of())
}
