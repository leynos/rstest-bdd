//! Scenario state helpers simplifying shared mutable data across steps.
//!
//! The [`Slot`] type wraps a `RefCell<Option<T>>`, exposing ergonomic helpers
//! for populating, inspecting, and clearing per-scenario values without
//! verbose boilerplate. The [`ScenarioState`] trait marks structs composed of
//! slots and provides a hook for clearing their contents between runs.
//!
//! # Examples
//!
//! ```
//! use rstest_bdd::state::Slot;
//!
//! let state = Slot::default();
//! assert!(state.is_empty());
//!
//! state.set("value");
//! assert_eq!(state.get(), Some("value"));
//! assert!(state.is_filled());
//! assert_eq!(state.take(), Some("value"));
//! assert!(state.is_empty());
//! ```
use std::cell::{RefCell, RefMut};

/// Shared scenario storage for a single value of type `T`.
///
/// A `Slot<T>` begins empty and can be filled, updated, and drained multiple
/// times during a scenario run. Internally it stores the payload in a
/// `RefCell<Option<T>>`, enabling interior mutability whilst remaining safe to
/// share between steps via immutable references.
pub struct Slot<T> {
    inner: RefCell<Option<T>>,
}

impl<T> Slot<T> {
    /// Construct an empty slot.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace the slot contents, returning the previous value when present.
    pub fn replace(&self, value: T) -> Option<T> {
        self.inner.replace(Some(value))
    }

    /// Store `value`, discarding any previous contents.
    pub fn set(&self, value: T) {
        let _ = self.replace(value);
    }

    /// Remove the current value from the slot.
    #[must_use]
    pub fn take(&self) -> Option<T> {
        self.inner.borrow_mut().take()
    }

    /// Borrow the value mutably, inserting one produced by `init` when empty.
    ///
    /// # Panics
    ///
    /// Panics if the slot is cleared between inserting the value and creating
    /// the returned borrow. This would require a reentrant call that violates
    /// the borrow rules.
    pub fn get_or_insert_with(&self, init: impl FnOnce() -> T) -> RefMut<'_, T> {
        let mut borrow = self.inner.borrow_mut();
        if borrow.is_none() {
            *borrow = Some(init());
        }
        RefMut::map(borrow, |opt| {
            opt.as_mut().map_or_else(
                || unreachable!("slot initialised immediately before mapping"),
                |value| value,
            )
        })
    }

    /// Read the current value by cloning it.
    #[must_use]
    pub fn get(&self) -> Option<T>
    where
        T: Clone,
    {
        self.inner.borrow().clone()
    }

    /// Apply `with_value` to the contained value if present.
    #[must_use]
    pub fn with_ref<R>(&self, with_value: impl FnOnce(&T) -> R) -> Option<R> {
        self.inner.borrow().as_ref().map(with_value)
    }

    /// Apply `with_value` to the contained value mutably if present.
    pub fn with_mut<R>(&self, with_value: impl FnOnce(&mut T) -> R) -> Option<R> {
        self.inner.borrow_mut().as_mut().map(with_value)
    }

    /// Return `true` when the slot holds a value.
    #[must_use]
    pub fn is_filled(&self) -> bool {
        self.inner.borrow().is_some()
    }

    /// Return `true` when the slot is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        !self.is_filled()
    }

    /// Remove the current value, leaving the slot empty.
    pub fn clear(&self) {
        let _ = self.take();
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for Slot<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Slot").field(&self.inner.borrow()).finish()
    }
}

impl<T> Default for Slot<T> {
    fn default() -> Self {
        Self {
            inner: RefCell::new(None),
        }
    }
}

/// Marker trait for structs composed of [`Slot`] fields.
///
/// Types deriving this trait gain a [`Default`] implementation that
/// initialises every slot to an empty state. The generated [`reset`](Self::reset)
/// method clears all slots, enabling tests to reuse shared fixtures between
/// scenarios when desired.
pub trait ScenarioState: Default {
    /// Clear every slot held by this scenario state.
    fn reset(&self);
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest_bdd_macros::ScenarioState as ScenarioStateDerive;

    #[test]
    fn slot_replaces_values() {
        let slot = Slot::default();
        assert!(slot.is_empty());
        slot.set(1);
        assert_eq!(slot.get(), Some(1));
        assert!(slot.is_filled());
        let previous = slot.replace(2);
        assert_eq!(previous, Some(1));
        assert_eq!(slot.get(), Some(2));
    }

    #[test]
    fn slot_get_or_insert_with_initialises() {
        let slot = Slot::default();
        {
            let mut value = slot.get_or_insert_with(|| String::from("hello"));
            value.push_str(" world");
        }
        assert_eq!(slot.get(), Some(String::from("hello world")));
    }

    #[test]
    fn slot_with_ref_and_with_mut_operate_conditionally() {
        let slot = Slot::default();
        assert_eq!(slot.with_ref(|value: &i32| *value), None);
        slot.set(5);
        let doubled = slot.with_ref(|value| value * 2);
        assert_eq!(doubled, Some(10));
        let mutated = slot.with_mut(|value| {
            *value += 1;
            *value
        });
        assert_eq!(mutated, Some(6));
        assert_eq!(slot.get(), Some(6));
    }

    #[derive(Debug, ScenarioStateDerive)]
    struct ExampleState {
        counter: Slot<u32>,
        label: Slot<String>,
    }

    #[test]
    fn scenario_state_reset_clears_all_slots() {
        let state = ExampleState::default();
        state.counter.set(7);
        state.label.set(String::from("active"));
        state.reset();
        assert!(state.counter.is_empty());
        assert!(state.label.is_empty());
    }
}
