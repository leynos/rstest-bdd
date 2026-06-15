//! Typed error surface for fixture borrowing.
//!
//! `StepContext` borrowing is guard-based (ADR-010): every borrow either
//! succeeds with a guard or fails with a [`FixtureBorrowError`] explaining
//! why. Borrow conflicts are reported as errors instead of `RefCell` panics
//! so step wrappers and adapters can surface precise diagnostics.

/// Reasons a fixture borrow can fail.
///
/// Returned by [`StepContext::try_borrow`](super::StepContext::try_borrow)
/// and [`StepContext::try_borrow_mut`](super::StepContext::try_borrow_mut).
///
/// # Examples
///
/// ```
/// use rstest_bdd::{FixtureBorrowError, StepContext};
///
/// let ctx = StepContext::default();
/// let err = ctx
///     .try_borrow::<u32>("missing")
///     .expect_err("borrowing an unknown fixture fails");
/// assert!(matches!(err, FixtureBorrowError::NotFound { .. }));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum FixtureBorrowError {
    /// No fixture or step-returned value is registered under the name.
    #[error("no fixture named '{name}' is available")]
    NotFound {
        /// The requested fixture name.
        name: String,
    },
    /// The entry exists but stores a different type than requested.
    #[error("fixture '{name}' does not store a value of the requested type")]
    TypeMismatch {
        /// The requested fixture name.
        name: String,
    },
    /// The fixture is currently borrowed in a way that conflicts with the
    /// request (mutably for any borrow, or shared for a mutable borrow).
    #[error("fixture '{name}' is already borrowed; drop the conflicting guard first")]
    AlreadyBorrowed {
        /// The requested fixture name.
        name: String,
    },
    /// The fixture was inserted by shared reference and cannot be borrowed
    /// mutably; insert it with `insert_owned` to enable mutation.
    #[error(
        "fixture '{name}' was inserted by shared reference and cannot be borrowed mutably; \
         insert it with `insert_owned` to enable mutation"
    )]
    NotMutable {
        /// The requested fixture name.
        name: String,
    },
}

impl FixtureBorrowError {
    pub(super) fn not_found(name: &str) -> Self {
        Self::NotFound { name: name.into() }
    }

    pub(super) fn type_mismatch(name: &str) -> Self {
        Self::TypeMismatch { name: name.into() }
    }

    pub(super) fn already_borrowed(name: &str) -> Self {
        Self::AlreadyBorrowed { name: name.into() }
    }

    pub(super) fn not_mutable(name: &str) -> Self {
        Self::NotMutable { name: name.into() }
    }
}
