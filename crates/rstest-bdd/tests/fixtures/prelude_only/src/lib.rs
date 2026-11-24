//! Standalone fixture crate for the V-2 prelude claim.
//!
//! Its purpose is its manifest and its test file: the package depends on
//! `rstest-bdd` and `rstest` and on nothing else, and `tests/prelude.rs` writes
//! a complete scenario through `rstest_bdd::prelude`. The library is
//! deliberately empty.
//!
//! `crates/rstest-bdd/tests/prelude_fixture.rs` builds and runs this crate, and
//! asserts the manifest above has not grown a `rstest-bdd-macros` entry, which
//! a lockfile refresh or a well-meaning "fix" could otherwise add while the
//! fixture kept passing.