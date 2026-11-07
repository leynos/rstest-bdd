//! Test-only access to shared step-registration state.

use super::REGISTERED;

/// Clear macro-registration state shared by external module tests.
pub(crate) fn clear_registered_steps_for_tests() {
    REGISTERED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clear();
}
