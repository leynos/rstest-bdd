//! Tests for step registration via macros

use rstest_bdd::{Step, iter};
use rstest_bdd_macros::{given, then, when};

#[given("a precondition")]
fn precondition() {}

#[when("an action")]
fn action() {}

#[then("a result")]
fn result() {}

#[test]
fn macros_register_steps() {
    let cases = [
        ("Given", "a precondition"),
        ("When", "an action"),
        ("Then", "a result"),
    ];

    for (keyword, pattern) in cases {
        assert!(
            iter::<Step>
                .into_iter()
                .any(|s| s.keyword == keyword && s.pattern == pattern),
            "Step not registered: {keyword} {pattern}"
        );
    }
}
