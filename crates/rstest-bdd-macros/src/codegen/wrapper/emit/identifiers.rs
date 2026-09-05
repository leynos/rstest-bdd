//! Identifier helpers for wrapper emission.
//!
//! Generates the internal wrapper, pattern, and fixture identifiers used by
//! emitted step wrappers. The identifiers are sanitized to ASCII to avoid
//! generating invalid symbols when step function names contain Unicode.
//!
//! [`COUNTER`] owns global wrapper-ID uniqueness across all generated
//! wrappers in the process. Allocation through [`next_wrapper_id`] and the
//! test-only reset through [`reset_wrapper_counter_for_tests`] share the same
//! narrow, test-aware mechanism: the reset exists solely for tests, is
//! compiled out of production builds, and callers must run under
//! `#[serial]` so parallel tests cannot race on the process-wide state.

use std::sync::atomic::{AtomicUsize, Ordering};

use proc_macro2::TokenStream as TokenStream2;
use quote::format_ident;

use crate::utils::ident::sanitize_ident;

/// Process-wide source of wrapper-ID uniqueness.
///
/// Every emitted wrapper suffix is allocated from this counter, so the value
/// it hands out must be unique for the lifetime of the process. The
/// test-only reset below is the sole writer other than [`next_wrapper_id`].
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

/// Resets the wrapper identifier counter to zero for a test.
///
/// This function exists **only for test code** so tests observe a
/// deterministic identifier sequence. Production code must never call it.
///
/// # Thread Safety
///
/// Rust tests run in parallel by default, and the counter is process-wide.
/// Every caller must run under `#[serial]` (see the tests in this module and
/// the codegen equivalence tests) so allocation and reset never race.
#[cfg(test)]
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

/// Generate the `StepPattern` constant used by a wrapper.
///
/// # Example
///
/// ```ignore
/// let pattern = syn::LitStr::new("^I log in$", proc_macro2::Span::call_site());
/// let ident = syn::Ident::new(
///     "__RSTEST_BDD_PATTERN_LOGIN_0",
///     proc_macro2::Span::call_site(),
/// );
/// let tokens = generate_wrapper_signature(&pattern, &ident);
/// assert!(tokens.to_string().contains("__RSTEST_BDD_PATTERN_LOGIN_0"));
/// ```
pub(in crate::codegen::wrapper::emit) fn generate_wrapper_signature(
    pattern: &syn::LitStr,
    pattern_ident: &proc_macro2::Ident,
) -> TokenStream2 {
    let path = crate::codegen::rstest_bdd_path();
    quote::quote! {
        static #pattern_ident: #path::StepPattern =
            #path::StepPattern::new(#pattern);
    }
}

/// Fetch and increment the global wrapper counter.
///
/// Returns the current counter value before incrementing. Uses relaxed ordering
/// since the counter only ensures a unique suffix and is not used for
/// synchronization with other data.
///
/// # Example
///
/// ```ignore
/// // Each call returns the next sequential ID.
/// let first = next_wrapper_id();   // e.g. 0
/// let second = next_wrapper_id();  // e.g. 1
/// assert_eq!(second, first + 1);
/// ```
pub(super) fn next_wrapper_id() -> usize { COUNTER.fetch_add(1, Ordering::Relaxed) }

#[cfg(test)]
mod tests {
    //! Serialised tests for the process-wide wrapper-ID counter.
    //!
    //! `COUNTER` is process-wide state, so allocation and reset must never
    //! overlap other tests that touch it. Every test here is `#[serial]`,
    //! matching the protocol used by the codegen equivalence tests.

    use serial_test::serial;

    use super::{next_wrapper_id, reset_wrapper_counter_for_tests};

    /// Resetting yields a deterministic sequence starting from zero.
    #[test]
    #[serial]
    fn reset_wrapper_counter_restart_ids_from_zero() {
        reset_wrapper_counter_for_tests();
        assert_eq!(next_wrapper_id(), 0);
        assert_eq!(next_wrapper_id(), 1);
        assert_eq!(next_wrapper_id(), 2);
    }

    /// A later reset restarts the sequence rather than resuming it.
    #[test]
    #[serial]
    fn reset_wrapper_counter_can_be_reapplied() {
        reset_wrapper_counter_for_tests();
        assert_eq!(next_wrapper_id(), 0);
        reset_wrapper_counter_for_tests();
        assert_eq!(next_wrapper_id(), 0);
    }
}
