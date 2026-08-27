//! Compile-time tests for the public workspace feature-discovery API.

/// Verifies callers handle the fallible `find_feature_files` result contract.
#[test]
fn find_feature_files_requires_result_handling() {
    let tests = trybuild::TestCases::new();
    tests.pass("tests/fixtures_workspace_discovery/find_feature_files_result.rs");
    tests.compile_fail("tests/fixtures_workspace_discovery/find_feature_files_vec.rs");
}
