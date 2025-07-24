//! Behavioural test for the greeting helper.

#[test]
fn greet_returns_expected_text() {
    assert_eq!(rstest_bdd::greet(), "Hello from rstest-bdd!");
}
