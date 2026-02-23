# rstest-bdd-harness-tokio

Tokio harness adapter and attribute policy for the `rstest-bdd` workspace.

This crate provides:

- `TokioHarness`, which executes scenario runners inside a Tokio
  current-thread runtime.
- `TokioAttributePolicy`, which emits `#[rstest::rstest]` and
  `#[tokio::test(flavor = "current_thread")]`.
