//! Standalone fixture crate for the 10.3.3 feature-file rebuild-invalidation
//! integration tests. The test suite copies this crate to
//! `target/tests/rebuild-invalidation/fixture` and mutates the copy; the
//! checked-in sources here are never modified at test time.
//!
//! The crate is deliberately a library with a test target (`tests/`), because
//! the regression tests resolve the artefacts Cargo produces for a test
//! binary (dep-info + fingerprints) rather than an `rlib` (whose metadata
//! retains source paths and constant values regardless of this mechanism).
