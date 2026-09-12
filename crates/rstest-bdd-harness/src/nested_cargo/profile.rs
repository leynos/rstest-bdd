//! Where a nested-`cargo` child writes its coverage profile.
//!
//! `cargo llvm-cov` merges the `.profraw` files named by the
//! `LLVM_PROFILE_FILE` pattern it exported, so replacing an inherited pattern
//! moves the child's coverage out of the run that would otherwise count it.
//! Callers state which destination they want, because the two callers of the
//! parent module want opposite things: a nested `cargo` build contributes
//! incidental coverage of the code under test, while the process under test
//! contributes exactly the coverage the run is measuring.

use std::{
    env,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

/// Where an instrumented child writes its coverage profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileDestination {
    /// Keep the caller's `LLVM_PROFILE_FILE` pattern, when one is inherited.
    ///
    /// Use for the process under test. Discarding its profile does not fail
    /// anything: the run stays green and the exercised lines merely report as
    /// unexecuted.
    Caller,
    /// Replace an inherited `LLVM_PROFILE_FILE` with a unique child-only
    /// scratch path.
    ///
    /// Use for a nested `cargo` invocation, whose incidental coverage must not
    /// merge into the caller's gated profile. Nothing is added when the caller
    /// is not instrumented.
    ChildScratch,
}

/// Counter for unique redirected coverage file names within this process.
static PROFILE_SEQ: AtomicU64 = AtomicU64::new(0);

/// Scratch root under the system temporary directory for redirected coverage.
fn profile_scratch_root() -> PathBuf {
    env::temp_dir().join(format!(
        "rstest-bdd-nested-cargo-profile-{}",
        std::process::id()
    ))
}

/// Create a unique child-only scratch destination for `LLVM_PROFILE_FILE`.
///
/// The name is unique per call (process id plus a monotonic counter), so
/// concurrent children never write the same `.profraw`.
pub(super) fn unique_profile_path() -> PathBuf {
    let base = profile_scratch_root();
    if let Err(err) = std::fs::create_dir_all(&base) {
        // A missing scratch directory is not fatal: the child then writes next
        // to its working directory, which is still child-only and never the
        // parent's gated profile.
        tracing::warn!(
            error = %err,
            "could not create nested-cargo coverage scratch directory"
        );
    }
    let seq = PROFILE_SEQ.fetch_add(1, Ordering::Relaxed);
    base.join(format!("nested-{seq}-%p.profraw"))
}
