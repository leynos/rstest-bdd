//! One budget shared by the commands that run before a timed body starts.
//!
//! A timed test that gates on a preparation pays for more than one subprocess
//! before it can begin: the fixture's dependency closure, then Cargo's identity
//! and the compiler's. Each has its own generous bound, and taking them in turn
//! means the worst case is their sum — which is how a query that succeeded just
//! inside its limit used to hand the probes a budget already spent. A query
//! that *failed* was safe, because the gate returned before the probes; the
//! slow success in between is the case that reached the runner's kill.
//!
//! [`Deadline`] is therefore created once by the caller that knows the runner's
//! period, and every command is charged against it. It lives here rather than
//! beside a command because it belongs to no single one of them: the closure
//! query and the toolchain probes are in different crates, and both take it.

use std::{
    io,
    time::{Duration, Instant},
};

use super::{
    Instruments,
    Preparation,
    lifecycle::reporter_of,
    toolchain::toolchain_identity_within,
};

/// One budget shared by every command that runs before a timed body starts.
///
/// The deadline is created once by the caller that knows the runner's period
/// and charges every command against it. [`remaining`] answers what is left,
/// and [`cap`] folds in the per-command limit, so an early command cannot spend
/// a later one's share and neither can overrun the whole.
///
/// This is a *cooperative* deadline over subprocesses, not a hard bound on the
/// gate: filesystem reads, process spawning and child cleanup are not charged.
/// Making those cancellable is a much larger change and is not what the
/// individual bounds were ever defending.
///
/// [`remaining`]: Deadline::remaining
/// [`cap`]: Deadline::cap
#[derive(Debug, Clone, Copy)]
pub struct Deadline {
    /// When the budget runs out.
    expires: Instant,
}

impl Deadline {
    /// Start a budget that lasts `budget` from now.
    #[must_use]
    pub fn starting_now(budget: Duration) -> Self {
        Self {
            expires: Instant::now() + budget,
        }
    }

    /// What is left of the budget, or `None` once it is spent.
    ///
    /// `None` rather than a zero duration, because the two mean different
    /// things to a caller about to spawn: zero would start a process with no
    /// time to run and report a timeout as though the command had failed, when
    /// the truth is that it was never started.
    #[must_use]
    pub fn remaining(&self) -> Option<Duration> {
        let left = self.expires.saturating_duration_since(Instant::now());
        (!left.is_zero()).then_some(left)
    }

    /// The bound to give the next command: the budget left, or `limit` if that
    /// is smaller.
    ///
    /// Both halves matter. The per-command limit keeps one slow command from
    /// eating the whole budget; the remaining time keeps a sequence of
    /// individually acceptable commands from overrunning the runner's period.
    ///
    /// # Errors
    ///
    /// Returns a [`io::ErrorKind::TimedOut`] error naming `what` when the
    /// budget is already spent, so a caller reports that it declined to start
    /// the command rather than that the command timed out.
    pub fn cap(&self, limit: Duration, what: &str) -> io::Result<Duration> {
        self.remaining().map_or_else(
            || {
                Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!(
                        "the preparation deadline expired before {what} could start; the runner \
                         would kill the test before this reported its own timeout"
                    ),
                ))
            },
            |left| Ok(left.min(limit)),
        )
    }
}

/// [`Instruments`] whose probe is charged against `deadline`.
///
/// Only a caller that has already spent part of one budget gets this: the timed
/// CLI test pays for the fixture's dependency closure before the gate runs, and
/// the probes must not then start with a fresh allowance of their own. Every
/// other family uses the gate's own `instruments_of`, whose probe carries its
/// own per-command limit and answer. Keeping the deadline out of `Instruments`
/// itself means the public field order and every existing caller are unchanged.
#[must_use]
pub fn instruments_within(
    preparation: &Preparation,
    deadline: Deadline,
) -> Instruments<impl FnOnce() -> io::Result<String> + '_, impl FnMut(&str)> {
    Instruments::new(
        move || {
            toolchain_identity_within(
                &preparation.environment,
                &preparation.working_directory,
                Some(deadline),
            )
        },
        reporter_of(),
    )
}

#[cfg(test)]
mod tests;
