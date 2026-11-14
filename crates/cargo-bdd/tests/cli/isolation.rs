//! The prefix test that keeps a developer's own configuration out of a child.
//!
//! `cargo-bdd` reads `CARGO_BDD_*` from the environment as a configuration
//! tier, so a smoke test that inherited the developer's own export would
//! assert against somebody else's setting rather than the file under test.
//! The filter that drops those variables needs one non-obvious property — the
//! comparison is case-insensitive on Windows and exact elsewhere — and this
//! module owns that property and its reasoning.
//!
//! # Why this is a separate module
//!
//! The filter's *policy* (which variables are dropped, and the platform
//! semantics behind that) is a different concern from the environment *map*
//! the sibling `environment` module asserts, and from the harness that applies
//! it. Keeping it here lets the two be read independently, and keeps the
//! test-target root inside its file-length budget.

use std::ffi::OsStr;

/// The prefix `cargo-bdd` reads its environment configuration from.
const CONFIGURATION_PREFIX: &str = "CARGO_BDD_";

/// Return whether `key` names a variable in the configuration prefix.
///
/// Windows variable names are case-insensitive but keep the spelling they were
/// set with, and `std::env::vars_os` returns that spelling. The binary looks up
/// `CARGO_BDD_LOG` literally (`crates/cargo-bdd/src/observability.rs`), and the
/// operating system resolves an inherited `cargo_bdd_log` to it, so an
/// exact-case test would forward a variable the binary genuinely reads. That
/// matters here because an inherited logging setting changes smoke-test output
/// on stderr, which is part of what those tests compare.
///
/// Elsewhere the spelling is what distinguishes the two names, so the test is
/// exact: `cargo_bdd_log` is a different variable from `CARGO_BDD_LOG` and is
/// not this binary's business.
#[must_use]
pub(crate) fn carries_configuration_prefix(key: &OsStr) -> bool {
    let key = key.to_string_lossy();
    if cfg!(windows) {
        // `get` rather than a slice because a lossily decoded name can hold a
        // multi-byte character at this offset, and a slice would panic on it.
        key.get(..CONFIGURATION_PREFIX.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(CONFIGURATION_PREFIX))
    } else {
        key.starts_with(CONFIGURATION_PREFIX)
    }
}
