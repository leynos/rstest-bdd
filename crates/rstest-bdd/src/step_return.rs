//! Type-directed normalization for macro-generated step return values.
//!
//! This hidden bridge is consumed only by wrappers emitted by
//! `rstest-bdd-macros`. Its inherent `Result` method must remain by-value and
//! keep its exact name: inherent-method precedence makes aliases resolve as
//! `Result` without allowing caller traits to silently reclassify them.

use core::{any::Any, fmt::Display};

/// Borrowing probe used to select a step return classification.
pub struct StepReturnProbe<'a, T: ?Sized>(pub &'a T);

/// Tag selected when the probed type is a `Result`.
pub struct StepReturnResultTag;

/// Tag selected when the probed type is not a `Result`.
pub struct StepReturnValueTag;

impl<T, E> StepReturnProbe<'_, Result<T, E>> {
    /// Select the `Result` normalization arm through inherent-method precedence.
    #[must_use]
    pub fn __rstest_bdd_step_return_kind(self) -> StepReturnResultTag { StepReturnResultTag }
}

/// Fallback selector for every value return, including non-dereferenced wrappers.
pub trait StepReturnValueKind {
    /// Select the value normalization arm.
    #[must_use]
    fn __rstest_bdd_step_return_kind(self) -> StepReturnValueTag;
}

impl<T: ?Sized> StepReturnValueKind for StepReturnProbe<'_, T> {
    fn __rstest_bdd_step_return_kind(self) -> StepReturnValueTag { StepReturnValueTag }
}

/// Error capability required to render a failed `Result` step.
#[diagnostic::on_unimplemented(
    message = "a step's error type `{Self}` must implement `std::fmt::Display`",
    label = "this step returns `Result<_, {Self}>`",
    note = "rstest-bdd renders a step's `Err` through `Display`"
)]
pub trait StepErrorDisplay: Display {}

#[diagnostic::do_not_recommend]
impl<E: Display + ?Sized> StepErrorDisplay for E {}

mod sealed {
    //! Sealed implementation detail for crate-owned dispatch tags.

    /// Seals normalization tags to crate-owned dispatch results.
    pub trait Sealed {}

    impl Sealed for super::StepReturnResultTag {}
    impl Sealed for super::StepReturnValueTag {}
}

/// Converts the selected tag and returned value into the wrapper result.
pub trait StepReturnNormalize<V>: sealed::Sealed {
    /// Normalize a selected step return into a payload or scenario failure.
    fn normalize(self, value: V) -> Result<Option<Box<dyn Any>>, String>;
}

impl<T: Any, E: StepErrorDisplay> StepReturnNormalize<Result<T, E>> for StepReturnResultTag {
    fn normalize(self, value: Result<T, E>) -> Result<Option<Box<dyn Any>>, String> {
        match value {
            Ok(value) => Ok(crate::__rstest_bdd_payload_from_value(value)),
            Err(error) => Err(error.to_string()),
        }
    }
}

impl<V: Any> StepReturnNormalize<V> for StepReturnValueTag {
    fn normalize(self, value: V) -> Result<Option<Box<dyn Any>>, String> {
        Ok(crate::__rstest_bdd_payload_from_value(value))
    }
}
