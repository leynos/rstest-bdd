//! Unique identifier generation for wrapper components.
//!
//! This module provides functionality for generating unique, deterministic
//! identifiers for sync/async wrapper functions, fixture arrays, and pattern
//! constants. A global counter ensures uniqueness across all generated wrappers.

use crate::utils::ident::sanitize_ident;
use quote::format_ident;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Identifiers for sync and async wrapper components.
///
/// Groups the four identifiers generated for each step wrapper to simplify
/// function signatures and reduce parameter counts.
pub(in crate::codegen::wrapper::emit) struct WrapperIdents {
    /// Identifier for the synchronous wrapper function.
    pub(in crate::codegen::wrapper::emit) sync_wrapper: proc_macro2::Ident,
    /// Identifier for the asynchronous wrapper function.
    pub(in crate::codegen::wrapper::emit) async_wrapper: proc_macro2::Ident,
    /// Identifier for the fixture array constant.
    pub(in crate::codegen::wrapper::emit) const_ident: proc_macro2::Ident,
    /// Identifier for the step pattern constant.
    pub(in crate::codegen::wrapper::emit) pattern_ident: proc_macro2::Ident,
}

/// Resets the wrapper identifier counter to zero.
///
/// This function is intended **only for test code** to ensure deterministic
/// identifier generation across test runs. Production code must never call
/// this function.
///
/// # Thread Safety
///
/// Rust tests run in parallel by default. Tests that call this function must
/// be serialised to avoid non-deterministic identifier generation. Use one of:
///
/// - The `#[serial]` attribute from the `serial_test` crate
/// - The `--test-threads=1` flag when running tests
/// - A shared mutex guard to coordinate access
///
/// # Example
///
/// ```ignore
/// use serial_test::serial;
///
/// #[test]
/// #[serial]
/// fn wrapper_identifiers_are_deterministic() {
///     reset_wrapper_counter_for_tests();
///     // First call returns 0, second returns 1, etc.
///     assert_eq!(next_wrapper_id(), 0);
///     assert_eq!(next_wrapper_id(), 1);
/// }
/// ```
// FIXME: https://github.com/leynos/rstest-bdd/issues/59 – utility for future golden tests
#[cfg(test)]
#[expect(dead_code, reason = "reserved for future golden tests (issue #59)")]
pub(crate) fn reset_wrapper_counter_for_tests() {
    // Use SeqCst ordering (rather than Relaxed used in production) to ensure
    // the reset is immediately visible to all threads. This is appropriate for
    // test setup where correctness matters more than performance.
    COUNTER.store(0, Ordering::SeqCst);
}

/// Generate unique identifiers for the wrapper components.
///
/// The provided step function identifier may contain Unicode. It is
/// sanitized to ASCII before constructing constant names to avoid emitting
/// invalid identifiers.
///
/// Returns identifiers for the sync wrapper function, async wrapper function,
/// fixture array constant, and pattern constant.
///
/// # Example
///
/// ```ignore
/// let ident: syn::Ident = syn::parse_str("my_step").expect("valid ident");
/// let ids = generate_wrapper_identifiers(&ident, 0);
///
/// assert_eq!(ids.sync_wrapper.to_string(), "__rstest_bdd_wrapper_my_step_0");
/// assert_eq!(ids.async_wrapper.to_string(), "__rstest_bdd_async_wrapper_my_step_0");
/// assert_eq!(ids.const_ident.to_string(), "__RSTEST_BDD_FIXTURES_MY_STEP_0");
/// assert_eq!(ids.pattern_ident.to_string(), "__RSTEST_BDD_PATTERN_MY_STEP_0");
/// ```
pub(in crate::codegen::wrapper::emit) fn generate_wrapper_identifiers(
    ident: &syn::Ident,
    id: usize,
) -> WrapperIdents {
    let ident_sanitized = sanitize_ident(&ident.to_string());
    let sync_wrapper = format_ident!("__rstest_bdd_wrapper_{}_{}", ident_sanitized, id);
    let async_wrapper = format_ident!("__rstest_bdd_async_wrapper_{}_{}", ident_sanitized, id);
    let ident_upper = ident_sanitized.to_ascii_uppercase();
    let const_ident = format_ident!("__RSTEST_BDD_FIXTURES_{}_{}", ident_upper, id);
    let pattern_ident = format_ident!("__RSTEST_BDD_PATTERN_{}_{}", ident_upper, id);
    WrapperIdents {
        sync_wrapper,
        async_wrapper,
        const_ident,
        pattern_ident,
    }
}

/// Fetch and increment the global wrapper counter.
///
/// Returns the current counter value before incrementing. Uses relaxed ordering
/// since the counter only ensures a unique suffix and is not used for
/// synchronisation with other data.
///
/// # Example
///
/// ```ignore
/// // Each call returns the next sequential ID.
/// let first = next_wrapper_id();   // e.g. 0
/// let second = next_wrapper_id();  // e.g. 1
/// assert_eq!(second, first + 1);
/// ```
pub(super) fn next_wrapper_id() -> usize {
    COUNTER.fetch_add(1, Ordering::Relaxed)
}
