//! Borrow guards handed to step functions for fixture access.
//!
//! Both guards keep any underlying `RefCell` borrow alive for the duration of
//! a step, so a step may hold a fixture reference across statements without
//! the borrow being released early.

use std::cell::{Ref, RefMut};

/// Borrowed fixture reference that keeps any underlying `RefCell` borrow alive
/// for the duration of a step.
pub enum FixtureRef<'a, T> {
    /// Reference bound directly to a shared fixture.
    Shared(&'a T),
    /// Borrow guard taken from a backing `RefCell`.
    Borrowed(Ref<'a, T>),
}

impl<T> FixtureRef<'_, T> {
    /// Access the borrowed value as an immutable reference.
    #[must_use]
    pub fn value(&self) -> &T {
        match self {
            Self::Shared(value) => value,
            Self::Borrowed(guard) => guard,
        }
    }
}

impl<T> AsRef<T> for FixtureRef<'_, T> {
    fn as_ref(&self) -> &T {
        self.value()
    }
}

/// Borrowed mutable fixture reference tied to the lifetime of the step borrow.
pub enum FixtureRefMut<'a, T> {
    /// Mutable reference produced by a prior step override.
    Override(&'a mut T),
    /// Borrow guard obtained from the underlying `RefCell`.
    Borrowed(RefMut<'a, T>),
}

impl<T> FixtureRefMut<'_, T> {
    /// Access the borrowed value mutably.
    #[must_use]
    pub fn value_mut(&mut self) -> &mut T {
        match self {
            Self::Override(value) => value,
            Self::Borrowed(guard) => guard,
        }
    }
}

impl<T> AsMut<T> for FixtureRefMut<'_, T> {
    fn as_mut(&mut self) -> &mut T {
        self.value_mut()
    }
}
