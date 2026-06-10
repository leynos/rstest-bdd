//! Opaque borrow guards handed out by `StepContext`.
//!
//! [`FixtureRef`] and [`FixtureRefMut`] keep the underlying `RefCell` borrow
//! alive for as long as the guard exists. Their internals are deliberately
//! private (ADR-010): whether a fixture is backed by a shared reference, an
//! owned cell, or a step-returned override is an implementation detail that
//! may change without breaking downstream code. Access the value through
//! `Deref`/`DerefMut`, [`FixtureRef::value`], or
//! [`FixtureRefMut::value_mut`].

use std::cell::{Ref, RefMut};
use std::ops::{Deref, DerefMut};

/// Borrowed fixture reference that keeps any underlying `RefCell` borrow
/// alive for the duration of a step.
///
/// Obtain via [`StepContext::try_borrow`](super::StepContext::try_borrow) or
/// [`StepContext::borrow_ref`](super::StepContext::borrow_ref). Multiple
/// shared guards for the same fixture may coexist; a shared guard conflicts
/// only with a mutable guard for the same fixture.
pub struct FixtureRef<'a, T>(FixtureRefInner<'a, T>);

enum FixtureRefInner<'a, T> {
    /// Reference bound directly to a shared fixture.
    Shared(&'a T),
    /// Borrow guard taken from a backing `RefCell`.
    Borrowed(Ref<'a, T>),
}

impl<'a, T> FixtureRef<'a, T> {
    pub(super) fn shared(value: &'a T) -> Self {
        Self(FixtureRefInner::Shared(value))
    }

    pub(super) fn borrowed(guard: Ref<'a, T>) -> Self {
        Self(FixtureRefInner::Borrowed(guard))
    }

    /// Access the borrowed value as an immutable reference.
    #[must_use]
    pub fn value(&self) -> &T {
        match &self.0 {
            FixtureRefInner::Shared(value) => value,
            FixtureRefInner::Borrowed(guard) => guard,
        }
    }
}

impl<T> Deref for FixtureRef<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.value()
    }
}

impl<T> AsRef<T> for FixtureRef<'_, T> {
    fn as_ref(&self) -> &T {
        self.value()
    }
}

/// Borrowed mutable fixture reference tied to the lifetime of the step borrow.
///
/// Obtain via
/// [`StepContext::try_borrow_mut`](super::StepContext::try_borrow_mut) or
/// [`StepContext::borrow_mut`](super::StepContext::borrow_mut). Guards for
/// *distinct* fixtures may be held concurrently; a second borrow of the same
/// fixture while a mutable guard is alive fails with
/// [`FixtureBorrowError::AlreadyBorrowed`](super::FixtureBorrowError::AlreadyBorrowed).
pub struct FixtureRefMut<'a, T>(RefMut<'a, T>);

impl<'a, T> FixtureRefMut<'a, T> {
    pub(super) fn borrowed(guard: RefMut<'a, T>) -> Self {
        Self(guard)
    }

    /// Access the borrowed value mutably.
    #[must_use]
    pub fn value_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<T> Deref for FixtureRefMut<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> DerefMut for FixtureRefMut<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<T> AsMut<T> for FixtureRefMut<'_, T> {
    fn as_mut(&mut self) -> &mut T {
        self.value_mut()
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for FixtureRef<'_, T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("FixtureRef")
            .field(self.value())
            .finish()
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for FixtureRefMut<'_, T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("FixtureRefMut")
            .field(&*self.0)
            .finish()
    }
}
