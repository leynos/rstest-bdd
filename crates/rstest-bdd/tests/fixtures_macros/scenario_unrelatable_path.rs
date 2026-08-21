//! Compile-fail fixture pinning the D4 diagnostic: a feature file path that
//! shares no filesystem root with `CARGO_MANIFEST_DIR` cannot be registered
//! as a Cargo rebuild dependency (Decision D4 in the 10.3.3 ExecPlan).
//!
//! The case is reachable on Windows via a different drive letter (and via UNC
//! versus drive-relative roots); on POSIX every absolute path shares one
//! root, so the fixture is registered only where it can fail — see the
//! `#[cfg(windows)]` registration in `crates/rstest-bdd/tests/trybuild_macros.rs`.
//! The diagnostic wording itself is pinned platform-independently by the
//! `untrackable_root_diagnostic_names_path_and_remedy` unit test in
//! `rstest-bdd-macros`.

use rstest_bdd_macros::scenario;

#[scenario(path = "Z:\\unrelatable\\x.feature")]
fn unreachable_root() {}

fn main() {}
