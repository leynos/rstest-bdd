//! Canary for the acceptance half of the same rule: an explicit
//! `#[test_attr(...)]` makes an `async` test acceptable to `rstest`.

#[rstest::rstest]
#[test_attr(tokio::test)]
async fn async_with_explicit_test_attr() {}

fn main() {}
