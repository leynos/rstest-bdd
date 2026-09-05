//! Standalone fixture crate for the 10.3.3 build-script addition test. The
//! test suite copies this crate to
//! `target/tests/feature-addition/fixture`, writes a `build.rs` into it from
//! the documented `scenarios-build-script` example, and adds a new `.feature`
//! file to the bound directory. Deliberately NO committed `build.rs`: the
//! recipe under test is written by the test itself.
