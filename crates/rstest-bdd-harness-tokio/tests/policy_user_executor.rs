//! Scenarios whose executor attribute is written by the user must keep
//! compiling.
//!
//! E001 fires only when the expansion has no executor at all. A third-party
//! policy paired with a user-written `#[tokio::test]`, and the default policy
//! paired with an explicit `#[test_attr(...)]`, both satisfy `rstest` and must
//! stay silent. `make test` compiles this file under `-D warnings`, so a stray
//! diagnostic breaks the build.

use rstest_bdd_harness::AttributePolicy;
use rstest_bdd_macros::{given, scenario, then, when};

mod third_party {
    //! A policy the macros cannot evaluate.

    use rstest_bdd_harness::{AttributePolicy, TestAttribute};

    pub struct Policy;

    const NO_ATTRIBUTES: [TestAttribute; 0] = [];

    impl AttributePolicy for Policy {
        fn test_attributes() -> &'static [TestAttribute] { &NO_ATTRIBUTES }
    }
}

#[given("a precondition")]
fn precondition() {}

#[when("an action occurs")]
fn action() {}

#[then("a result is produced")]
fn result() {}

/// The fixture's premise: this policy really does add no executor itself.
#[test]
fn the_third_party_policy_adds_no_attributes() {
    assert!(
        third_party::Policy::test_attributes().is_empty(),
        "the third-party policy exists to add no attribute of its own"
    );
}

/// A third-party policy plus a user-written executor compiles and runs.
#[scenario(path = "tests/fixtures_macros/basic.feature", attributes = third_party::Policy)]
#[tokio::test]
async fn foreign_policy_with_user_executor() {}

/// The default policy plus `#[test_attr(...)]` compiles and runs.
#[scenario(
    path = "tests/fixtures_macros/basic.feature",
    attributes = rstest_bdd_harness::DefaultAttributePolicy,
)]
#[test_attr(tokio::test)]
async fn default_policy_with_explicit_test_attr() {}
