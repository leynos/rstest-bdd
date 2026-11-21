//! Compile-time tests for the public workspace feature-discovery API.

#[path = "workspace_discovery_compile/preparation.rs"]
mod preparation;

/// Verifies callers handle the fallible `find_feature_files` result contract.
#[test]
fn find_feature_files_requires_result_handling() {
    // Under nextest a missing or stale preparation stamp fails here with the
    // command that prepares it, rather than making this deadline pay for a
    // cold dependency build; under plain `cargo test` the gate prepares
    // lazily under its own lock.
    preparation::tree()
        .ensure_prepared(preparation::prepare)
        .expect("the server trybuild tree should be prepared before the timed case");
    let tests = trybuild::TestCases::new();
    tests.pass("tests/fixtures_workspace_discovery/find_feature_files_result.rs");
    tests.compile_fail("tests/fixtures_workspace_discovery/find_feature_files_vec.rs");
}
