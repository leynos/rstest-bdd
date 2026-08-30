//! Shared fixture values for step-return behavioural scenarios.

use rstest::fixture;

/// Numeric fixture used by the step-return scenarios.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Number(pub(super) i32);

/// First competing fixture type used to test override ambiguity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PrimaryValue(pub(super) i32);

/// Second competing fixture type used to test override ambiguity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SecondaryValue(pub(super) i32);

/// Provides the initial numeric fixture for step-return scenarios.
#[rstest_bdd_test_macros::allow_fixture_expansion_lints]
#[fixture]
pub(super) fn number() -> Number { Number(1) }
