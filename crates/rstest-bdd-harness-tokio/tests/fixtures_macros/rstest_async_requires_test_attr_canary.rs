//! Canary for the `rstest` rule E001 mirrors: an `async` test with no
//! executor attribute is rejected by `rstest` itself.

#[rstest::rstest]
async fn async_without_executor() {}

fn main() {}
