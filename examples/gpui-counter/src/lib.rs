//! Example counter application for demonstrating GPUI harness integration
//! with rstest-bdd.
//!
//! The library models a simple counter with an observation record for GPUI
//! context details. Fixtures share the counter across steps via interior
//! mutability, following the same pattern as the `japanese-ledger` example.

use std::cell::Cell;

/// A simple counter that also records observations from the GPUI test
/// harness context.
///
/// All fields use [`Cell`] for interior mutability so that BDD step
/// definitions can share a single `&CounterApp` across Given/When/Then
/// steps without requiring `&mut` access.  This is safe because scenario
/// execution is single-threaded; `CounterApp` intentionally does *not*
/// implement `Sync`.
///
/// # Examples
///
/// ```
/// use gpui_counter::CounterApp;
///
/// let app = CounterApp::new(0);
/// app.increment(5);
/// app.decrement(2);
/// assert_eq!(app.value(), 3);
///
/// // Amounts are unsigned (`u32`), so the direction is always unambiguous.
/// app.set_value(-1);
/// app.increment(3);
/// assert_eq!(app.value(), 2);
/// ```
#[derive(Debug, Default)]
pub struct CounterApp {
    value: Cell<i32>,
    dispatcher_seed: Cell<Option<u64>>,
}

impl CounterApp {
    /// Creates a counter initialized to the given starting value.
    #[must_use]
    pub fn new(start: i32) -> Self {
        Self {
            value: Cell::new(start),
            dispatcher_seed: Cell::new(None),
        }
    }

    /// Returns the current counter value.
    #[must_use]
    pub fn value(&self) -> i32 {
        self.value.get()
    }

    /// Replaces the stored counter value with the provided amount.
    pub fn set_value(&self, amount: i32) {
        self.value.set(amount);
    }

    /// Increases the counter by `amount`, saturating at `i32::MAX`.
    ///
    /// The amount is unsigned so the direction is always unambiguous.
    pub fn increment(&self, amount: u32) {
        let delta = i64::from(self.value.get()) + i64::from(amount);
        self.value.set(saturate_to_i32(delta));
    }

    /// Decreases the counter by `amount`, saturating at `i32::MIN`.
    ///
    /// The amount is unsigned so the direction is always unambiguous.
    pub fn decrement(&self, amount: u32) {
        let delta = i64::from(self.value.get()) - i64::from(amount);
        self.value.set(saturate_to_i32(delta));
    }

    /// Records an observed GPUI dispatcher seed.
    pub fn record_dispatcher_seed(&self, seed: u64) {
        self.dispatcher_seed.set(Some(seed));
    }

    /// Returns the last recorded dispatcher seed, if any.
    #[must_use]
    pub fn dispatcher_seed(&self) -> Option<u64> {
        self.dispatcher_seed.get()
    }
}

/// Clamps an `i64` value to the `i32` range.
fn saturate_to_i32(value: i64) -> i32 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "value is clamped to i32 range before truncation"
    )]
    {
        value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
    }
}

#[cfg(test)]
mod tests {
    //! Tests for `CounterApp` behavior.

    use super::CounterApp;
    use rstest::{fixture, rstest};

    #[fixture]
    fn counter() -> CounterApp {
        CounterApp::new(0)
    }

    #[rstest]
    fn starts_at_zero(counter: CounterApp) {
        assert_eq!(counter.value(), 0);
    }

    #[rstest]
    fn increments_value(counter: CounterApp) {
        counter.increment(5);
        assert_eq!(counter.value(), 5);
    }

    #[rstest]
    fn decrements_value(counter: CounterApp) {
        counter.increment(10);
        counter.decrement(3);
        assert_eq!(counter.value(), 7);
    }

    #[rstest]
    fn saturates_on_overflow(counter: CounterApp) {
        counter.increment(u32::MAX);
        counter.increment(1);
        assert_eq!(counter.value(), i32::MAX);
    }

    #[rstest]
    fn saturates_on_underflow(counter: CounterApp) {
        counter.decrement(u32::MAX);
        counter.decrement(u32::MAX);
        assert_eq!(counter.value(), i32::MIN);
    }

    #[rstest]
    fn records_dispatcher_seed(counter: CounterApp) {
        assert!(counter.dispatcher_seed().is_none());
        counter.record_dispatcher_seed(42);
        assert_eq!(counter.dispatcher_seed(), Some(42));
    }
}
