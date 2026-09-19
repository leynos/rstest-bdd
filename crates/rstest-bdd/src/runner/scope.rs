//! The run token: a single-use borrow of a caller's context, with cleanup.
//!
//! [`ScenarioScope`] exists to make two things true by construction rather
//! than by discipline. It borrows the caller's [`StepContext`] for the whole
//! run, so a run cannot outlive the fixtures it borrows; and it reaches that
//! context *only through* [`CleanupGuard`], so no run can observe a context
//! whose cleanup has been detached from it.
//!
//! The split between the two types is load-bearing. If `ScenarioScope` were
//! itself `Drop`, a future `with_hooks` could not move its fields out, because
//! a type with a destructor cannot be partially moved. Deferring that method
//! (D2 option (ii)) does not remove the reason for the split: the guard is what
//! keeps cleanup unconditional, and holding the *only* accessor to the context
//! is what keeps it from being bypassed rather than merely being likely to run.
//!
//! Cleanup clears step-returned override values and nothing else. Reusing one
//! `StepContext` across scenarios is therefore **not supported**: the scope
//! cannot reset fixture cells the caller owns, so a reused context gives
//! partial isolation, which is worse than none.

use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::{StepContext, config};

/// The default lifecycle hooks: both hooks succeed and do nothing.
///
/// Shipped now, with no `Lifecycle` trait to implement, because
/// `ScenarioScope`'s type parameter defaults to it. The defaulted parameter is
/// what makes adding the trait later source-compatible: every existing
/// `ScenarioScope::new(&mut ctx)` keeps working unchanged, whereas shipping the
/// scope without a parameter would have forced every call site to be rewritten
/// when the hooks arrived. See D2 option (ii).
#[derive(Debug, Default, Clone, Copy)]
pub struct NoHooks;

/// Owns the borrow of the run's context, and clears it on drop.
///
/// Holds the context mutably for as long as it lives, and is the only route to
/// it: [`ScenarioScope`] exposes no second path, so a driver cannot obtain the
/// context in a form that outlives cleanup. Dropping it clears every
/// step-returned override value, catching a panic from a value's destructor so
/// that a panicking drop degrades to a warning rather than aborting the
/// process — which, during another unwind, is the difference between a report
/// and exit 134.
///
/// There is deliberately no flag to skip cleanup. The plan's lifecycle matrix
/// has no row on which override values must survive the run, and an unexercised
/// branch inside a destructor is the hardest kind of code to keep honest. The
/// flag arrives with the first hook that needs it, not before.
pub(super) struct CleanupGuard<'ctx, 'fix> {
    /// The context whose override values are cleared on drop.
    ctx: &'ctx mut StepContext<'fix>,
}

impl<'ctx, 'fix> CleanupGuard<'ctx, 'fix> {
    /// Take ownership of the borrow that cleanup will end.
    pub(super) const fn new(ctx: &'ctx mut StepContext<'fix>) -> Self { Self { ctx } }

    /// Borrow the context for a run.
    ///
    /// The only accessor, and on the guard rather than on the scope, so that
    /// "cleanup cannot be skipped" holds of the access path itself: every
    /// borrow made through here is still owned by the value that will clear it.
    pub(super) fn ctx_mut(&mut self) -> &mut StepContext<'fix> { self.ctx }
}

impl Drop for CleanupGuard<'_, '_> {
    fn drop(&mut self) {
        // `StepContext` is full of `RefCell`s and is not `UnwindSafe`, so the
        // assertion is required. It is sound in the sense that matters here: a
        // panic mid-clear leaves *fewer* values, never a half-visible one,
        // because clearing drops the map whole.
        let outcome = catch_unwind(AssertUnwindSafe(|| self.ctx.clear_values()));
        if let Err(payload) = outcome {
            tracing::warn!(
                reason = %crate::panic_message(payload.as_ref()),
                "a step-returned value panicked while being dropped during cleanup",
            );
        }
    }
}

/// A single-use lifecycle token owning the cleanup guard around a
/// caller-supplied [`StepContext`].
///
/// Dropping the scope clears the run's step-returned override values. That
/// cleanup is synchronous, so it survives cancellation of an asynchronous run;
/// an awaited after hook would not, which is one reason the hooks are deferred.
///
/// Reusing one `StepContext` across scenarios is **not supported**: the scope
/// clears override values but cannot reset fixture cells the caller owns, so a
/// reused context gives partial isolation, which is worse than none.
///
/// # Examples
///
/// ```
/// use rstest_bdd::{StepContext, runner::ScenarioScope};
///
/// let mut ctx = StepContext::default();
/// let scope = ScenarioScope::new(&mut ctx);
/// drop(scope); // cleanup runs here
/// ```
pub struct ScenarioScope<'ctx, 'fix, H = NoHooks> {
    /// Cleanup, carried so it cannot be skipped or bypassed.
    guard: CleanupGuard<'ctx, 'fix>,
    /// `fail_on_skipped`, resolved once at construction.
    ///
    /// Stored rather than re-read, so a run cannot observe the process-global
    /// configuration changing part-way through it. The plan's own
    /// `allow_skipped` is folded in later, by the runner, which is the first
    /// place both inputs are in hand.
    fail_on_skipped: bool,
    /// Makes the hook type parameter part of the type without storing one.
    ///
    /// Deferred hooks mean there is nothing to store; the parameter stays so
    /// that adding them is source-compatible.
    hooks: std::marker::PhantomData<H>,
}

impl<'ctx, 'fix> ScenarioScope<'ctx, 'fix, NoHooks> {
    /// Wrap a context for one scenario run, resolving skip policy once.
    ///
    /// [`config::fail_on_skipped`] is read exactly here and nowhere else in the
    /// runner, so every step of a run sees the same policy even if the global
    /// changes underneath it.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd::{StepContext, runner::ScenarioScope};
    ///
    /// let mut ctx = StepContext::default();
    /// let scope = ScenarioScope::new(&mut ctx);
    /// drop(scope);
    /// ```
    #[must_use]
    pub fn new(ctx: &'ctx mut StepContext<'fix>) -> Self {
        Self {
            guard: CleanupGuard::new(ctx),
            fail_on_skipped: config::fail_on_skipped(),
            hooks: std::marker::PhantomData,
        }
    }
}

impl<'fix, H> ScenarioScope<'_, 'fix, H> {
    /// Override the resolved skip policy for this run, bypassing the global.
    ///
    /// The argument is `fail_on_skipped` alone: the plan's own `allow_skipped`
    /// is not known until the runner is handed a plan, and the two are folded
    /// together there.
    ///
    /// # Examples
    ///
    /// ```
    /// use rstest_bdd::{StepContext, runner::ScenarioScope};
    ///
    /// let mut ctx = StepContext::default();
    /// let scope = ScenarioScope::new(&mut ctx).with_skip_policy(false);
    /// drop(scope);
    /// ```
    #[must_use]
    pub fn with_skip_policy(mut self, fail_on_skipped: bool) -> Self {
        self.fail_on_skipped = fail_on_skipped;
        self
    }

    /// Borrow everything a run needs from this scope.
    ///
    /// Disjoint field borrows, expressible only through direct field access on
    /// one `&mut self` — which is why this is a method here rather than a pair
    /// of accessors. Returns the policy and the context together so a driver
    /// cannot hold one without the other.
    pub(super) fn split(&mut self) -> (bool, &mut StepContext<'fix>) {
        (self.fail_on_skipped, self.guard.ctx_mut())
    }
}
