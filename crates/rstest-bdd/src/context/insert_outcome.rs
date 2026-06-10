//! Outcome type for `StepContext::insert_value`.
//!
//! Distinguishes a recorded step-return override from the two cases where
//! the value is dropped (no matching fixture type, or an ambiguous match),
//! which a bare `Option` return previously conflated.

use std::any::Any;

/// Outcome of [`StepContext::insert_value`].
///
/// Distinguishes a recorded override from the two cases where the value is
/// dropped, which a bare `Option` return previously conflated.
#[derive(Debug)]
#[must_use = "inspect the outcome to detect dropped step return values"]
pub enum InsertOutcome {
    /// The value was recorded as an override for the uniquely matching
    /// fixture; carries the previous override when one existed.
    Inserted(Option<Box<dyn Any>>),
    /// No fixture matches the value's type; the value was dropped.
    NoMatch,
    /// More than one fixture matches the value's type; the value was dropped
    /// to avoid an ambiguous override (a warning is emitted).
    AmbiguousIgnored,
}

impl InsertOutcome {
    /// Whether the value was recorded as an override.
    #[must_use]
    pub const fn is_inserted(&self) -> bool {
        matches!(self, Self::Inserted(_))
    }

    /// Consume the outcome and return the displaced previous override, when
    /// the insert succeeded and one existed.
    #[must_use]
    pub fn into_previous(self) -> Option<Box<dyn Any>> {
        match self {
            Self::Inserted(previous) => previous,
            Self::NoMatch | Self::AmbiguousIgnored => None,
        }
    }
}
