# rstest-bdd-harness-gpui

GPUI harness adapter and attribute policy for the `rstest-bdd` workspace.

This crate provides:

- `GpuiHarness`, which executes scenario runners inside GPUI's test harness.
- `GpuiAttributePolicy`, which emits `#[rstest::rstest]` and `#[gpui::test]`.
