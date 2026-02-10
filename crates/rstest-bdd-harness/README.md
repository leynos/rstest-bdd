# rstest-bdd-harness

Harness adapter and attribute policy interfaces for the `rstest-bdd` workspace.

This crate defines:

- `HarnessAdapter` and shared scenario runner types.
- `StdHarness`, the default synchronous harness implementation.
- `AttributePolicy` and `DefaultAttributePolicy`, which emits only
  `#[rstest::rstest]`.
